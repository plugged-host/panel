//! plugged.host: neo's agent loop, one unit of work per call (docs/features/neo.md).
//!
//! The chat drives it: [`Agent::send`] starts a turn, then [`Agent::step`] is called until it
//! stops answering `continue`. Each step either runs one queued tool or makes one model call, and
//! everything is stored as it goes, so no request holds a worker for a whole conversation and a
//! turn resumes after the person navigates away.
//!
//! Every step checks again that neo is on, that the person still has it on and agreed to it, and
//! that they may still use this server; every tool checks its permissions again right before it
//! runs ([`super::tools::permitted`]).

use super::{
    Ask, DeltaSender, TurnError,
    conversations::{Conversation, Message, NewMessage, Preferences, role, status, tool_status},
    tools::{self, ToolContext, ToolError, ToolResult, ToolRisk, ToolScope},
};
use crate::models::{
    server::Server,
    user::{PermissionManager, User},
};
use serde::Serialize;
use serde_json::{Value, json};
use utoipa::ToSchema;

/// The longest message a person can send.
pub const MAX_MESSAGE_CHARS: usize = 4000;
/// How long one step may hold its conversation.
const STEP_LOCK_SECONDS: u64 = 120;

/// What the chat should do next: `continue` and `busy` mean "step again" (after `retry_after`
/// seconds for busy); everything else ends the loop until the person acts.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Continue,
    Busy,
    AwaitingApproval,
    Done,
    Limited,
    Unlinked,
    Error,
    Blocked,
    Redirect,
}

#[derive(ToSchema, Serialize, Clone, Debug, PartialEq)]
pub struct StepResult {
    pub state: StepState,
    /// Why, for the chat to say in the person's language (`errors.<code>`), e.g. `rate_limited`.
    pub code: Option<&'static str>,
    /// Values for the code's text, e.g. `{"retry_at": "…"}`.
    #[schema(value_type = Object)]
    pub params: Value,
    pub retry_after: u64,
    /// Where the person is taken (`redirect`): a start gate's page.
    pub url: Option<String>,
}

impl StepResult {
    fn of(state: StepState) -> Self {
        Self {
            state,
            code: None,
            params: json!({}),
            retry_after: 0,
            url: None,
        }
    }

    fn with(state: StepState, code: &'static str, params: Value) -> Self {
        Self {
            code: Some(code),
            params,
            ..Self::of(state)
        }
    }

    pub fn proceed() -> Self {
        Self::of(StepState::Continue)
    }

    pub fn done() -> Self {
        Self::of(StepState::Done)
    }

    pub fn busy(retry_after: u64) -> Self {
        Self {
            retry_after: retry_after.max(1),
            ..Self::of(StepState::Busy)
        }
    }

    pub fn blocked(code: &'static str) -> Self {
        Self::with(StepState::Blocked, code, json!({}))
    }

    pub fn error(code: &'static str) -> Self {
        Self::with(StepState::Error, code, json!({}))
    }

    fn redirect(url: String) -> Self {
        Self {
            url: Some(url),
            ..Self::of(StepState::Redirect)
        }
    }
}

/// A turn that starts with a read tool already decided (the log check's "Ask neo" starts with
/// `read_mclogs`): stored as if the model had called it, so the model starts from the result,
/// fenced as untrusted like any tool output, instead of log text pasted into the person's message.
pub struct Seed {
    pub tool: &'static str,
    pub arguments: Value,
}

/// neo working for one person in one place: a server's chat, or the account chat.
pub struct Agent<'a> {
    pub state: &'a crate::State,
    pub user: &'a User,
    /// The server of a server chat, loaded for the user (with their subuser permissions).
    pub server: Option<&'a Server>,
    /// The user's permissions, scoped to `server` when there is one.
    pub permissions: &'a PermissionManager,
    pub ip: std::net::IpAddr,
}

impl Agent<'_> {
    fn context(&self, conversation: &Conversation) -> ToolContext<'_> {
        ToolContext {
            state: self.state,
            user: self.user,
            server: self.server,
            permissions: self.permissions,
            conversation: conversation.uuid,
            ip: self.ip,
        }
    }

    /// The open conversation here, created on first use.
    pub async fn conversation(&self) -> Result<Conversation, sqlx::Error> {
        Conversation::open_for(
            &self.state.database,
            self.user.uuid,
            self.server.map(|server| server.uuid),
        )
        .await
    }

    /// Why the person cannot use neo here right now.
    pub async fn blocked(
        &self,
        conversation: &Conversation,
    ) -> Result<Option<&'static str>, anyhow::Error> {
        if conversation.user_uuid != self.user.uuid
            || conversation.server_uuid != self.server.map(|server| server.uuid)
        {
            return Ok(Some("not_yours"));
        }

        if !self.state.settings.get().await?.neo.enabled {
            return Ok(Some("disabled"));
        }

        let preferences = Preferences::of(&self.state.database, self.user.uuid).await?;
        if !preferences.enabled {
            return Ok(Some("turned_off"));
        }
        if preferences.acknowledged.is_none() {
            return Ok(Some("consent"));
        }

        if let Some(server) = self.server
            && !tools::may_use_on(self.user, server)
        {
            return Ok(Some("no_access"));
        }

        Ok(None)
    }

    /// Starts a turn with the person's message (and its images, already shrunk JPEGs).
    pub async fn send(
        &self,
        conversation: &mut Conversation,
        text: &str,
        images: Vec<Vec<u8>>,
        seed: Option<Seed>,
    ) -> Result<StepResult, anyhow::Error> {
        if let Some(code) = self.blocked(conversation).await? {
            return Ok(StepResult::blocked(code));
        }

        let text: String = text.trim().chars().take(MAX_MESSAGE_CHARS).collect();
        if text.is_empty() && images.is_empty() {
            return Ok(StepResult::done());
        }

        *conversation = conversation.refresh(&self.state.database).await?;
        if conversation.status == status::RUNNING {
            return Ok(StepResult::busy(2));
        }

        let messages_per_minute = self
            .state
            .settings
            .get()
            .await?
            .neo
            .messages_per_minute
            .max(1);
        if self
            .state
            .cache
            .ratelimit(
                "neo::messages",
                messages_per_minute as u64 + 1,
                60,
                self.user.uuid.to_string(),
            )
            .await
            .is_err()
        {
            return Ok(StepResult::error("rate_limited"));
        }

        // a seed is a read tool offered here, so it skips no approval
        let seed = match seed {
            Some(seed) => {
                let context = self.context(conversation);
                let tool = tools::available(&context)
                    .into_iter()
                    .find(|tool| tool.name() == seed.tool)
                    .filter(|tool| tool.risk(&seed.arguments) == ToolRisk::Read);
                match tool {
                    Some(tool) => Some((tool, seed.arguments)),
                    None => return Ok(StepResult::error("unavailable")),
                }
            }
            None => None,
        };

        let mut transaction = self.state.database.write().begin().await?;

        // anything still waiting belongs to the previous request: answer it, so every tool call
        // keeps a result (providers refuse a transcript with an unanswered call)
        sqlx::query(
            "UPDATE neo_messages SET status = 'skipped', content = 'Not run: the user sent a new message instead.'
            WHERE conversation_uuid = $1 AND role = 'tool' AND status = ANY($2)",
        )
        .bind(conversation.uuid)
        .bind(tool_status::WAITING)
        .execute(&mut *transaction)
        .await?;

        let message = conversation
            .append(
                &mut *transaction,
                role::USER,
                NewMessage {
                    content: Some(text.clone()),
                    ..Default::default()
                },
            )
            .await?;
        if !images.is_empty() {
            let uuids = super::images::store(&mut transaction, message.uuid, &images).await?;
            sqlx::query("UPDATE neo_messages SET meta = $2 WHERE uuid = $1")
                .bind(message.uuid)
                .bind(json!({"images": uuids}))
                .execute(&mut *transaction)
                .await?;
        }

        if let Some((tool, arguments)) = seed {
            let id = super::chat::new_call_id();
            conversation
                .append(
                    &mut *transaction,
                    role::ASSISTANT,
                    NewMessage {
                        tool_calls: Some(json!([{
                            "id": id,
                            "type": "function",
                            "function": {"name": tool.name(), "arguments": arguments.to_string()},
                        }])),
                        meta: Some(json!({"seeded": true})),
                        ..Default::default()
                    },
                )
                .await?;
            conversation
                .append(
                    &mut *transaction,
                    role::TOOL,
                    NewMessage {
                        tool_call_id: Some(id),
                        tool_name: Some(tool.name().to_string()),
                        status: Some(tool_status::QUEUED),
                        meta: Some(json!({
                            "arguments": arguments,
                            "summary": tool.summarize(&arguments),
                            "risk": ToolRisk::Read,
                            "approval": null,
                            "preview": null,
                        })),
                        ..Default::default()
                    },
                )
                .await?;
        }

        let title = (!text.is_empty()).then(|| title_of(&text));
        sqlx::query(
            "UPDATE neo_conversations SET title = COALESCE(title, $2), status = 'running', step_count = 0,
                last_error = NULL, last_activity = NOW()
            WHERE uuid = $1",
        )
        .bind(conversation.uuid)
        .bind(title)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        conversation.status = status::RUNNING.to_string();

        Ok(StepResult::proceed())
    }

    /// One unit of work: the next queued tool, or one model call. With `deltas` the model's
    /// answer is handed on while it is written.
    pub async fn step(
        &self,
        conversation: &mut Conversation,
        deltas: Option<&DeltaSender>,
    ) -> Result<StepResult, anyhow::Error> {
        let Ok(_lock) = self
            .state
            .cache
            .lock(
                format!("neo::conversation::{}", conversation.uuid),
                Some(STEP_LOCK_SECONDS),
                Some(1),
            )
            .await
        else {
            return Ok(StepResult::busy(1));
        };

        *conversation = conversation.refresh(&self.state.database).await?;
        if let Some(code) = self.blocked(conversation).await? {
            return Ok(StepResult::blocked(code));
        }
        if conversation.status != status::RUNNING {
            return Ok(state_of(conversation));
        }

        let queued = conversation
            .waiting_tools(
                &self.state.database,
                &[tool_status::QUEUED, tool_status::APPROVED],
            )
            .await?;
        if let Some(next) = queued.into_iter().next() {
            return self.run_tool(conversation, next).await;
        }

        if !conversation
            .waiting_tools(&self.state.database, &[tool_status::PENDING_APPROVAL])
            .await?
            .is_empty()
        {
            conversation
                .set_status(&self.state.database, status::AWAITING_APPROVAL)
                .await?;

            return Ok(StepResult::of(StepState::AwaitingApproval));
        }

        self.call_model(conversation, deltas).await
    }

    /// Approves one waiting call, or all of them.
    pub async fn approve(
        &self,
        conversation: &mut Conversation,
        message: Option<uuid::Uuid>,
    ) -> Result<StepResult, anyhow::Error> {
        self.decide(conversation, message, true).await
    }

    pub async fn deny(
        &self,
        conversation: &mut Conversation,
        message: uuid::Uuid,
    ) -> Result<StepResult, anyhow::Error> {
        self.decide(conversation, Some(message), false).await
    }

    /// Stops the current request: nothing still waiting runs.
    pub async fn cancel(
        &self,
        conversation: &mut Conversation,
    ) -> Result<StepResult, anyhow::Error> {
        conversation
            .skip_waiting(
                &self.state.database,
                "Not run: the user stopped this request.",
            )
            .await?;
        conversation
            .set_status(&self.state.database, status::IDLE)
            .await?;

        Ok(StepResult::done())
    }

    /// Tries again after an outage or once a limit window has reset.
    pub async fn retry(
        &self,
        conversation: &mut Conversation,
    ) -> Result<StepResult, anyhow::Error> {
        *conversation = conversation.refresh(&self.state.database).await?;
        if ![status::ERROR, status::LIMITED, status::UNLINKED]
            .contains(&conversation.status.as_str())
        {
            return Ok(state_of(conversation));
        }
        if let Some(code) = self.blocked(conversation).await? {
            return Ok(StepResult::blocked(code));
        }

        sqlx::query(
            "UPDATE neo_conversations SET status = 'running', last_error = NULL, last_activity = NOW()
            WHERE uuid = $1",
        )
        .bind(conversation.uuid)
        .execute(self.state.database.write())
        .await?;
        conversation.status = status::RUNNING.to_string();

        Ok(StepResult::proceed())
    }

    async fn decide(
        &self,
        conversation: &mut Conversation,
        message: Option<uuid::Uuid>,
        approve: bool,
    ) -> Result<StepResult, anyhow::Error> {
        if let Some(code) = self.blocked(conversation).await? {
            return Ok(StepResult::blocked(code));
        }

        let pending = conversation
            .waiting_tools(&self.state.database, &[tool_status::PENDING_APPROVAL])
            .await?;
        for mut row in pending
            .into_iter()
            .filter(|row| message.is_none_or(|uuid| row.uuid == uuid))
        {
            if approve {
                let meta = row.meta_with(json!({
                    "approval": "user",
                    "approved_at": chrono::Utc::now().to_rfc3339(),
                }));
                row.update_tool(
                    self.state.database.write(),
                    tool_status::APPROVED,
                    None,
                    meta,
                )
                .await?;
            } else {
                let meta = row.meta_with(json!({"approval": "denied"}));
                row.update_tool(
                    self.state.database.write(),
                    tool_status::DENIED,
                    Some("The user declined this action.".into()),
                    meta,
                )
                .await?;
            }
        }

        let still_pending = !conversation
            .waiting_tools(&self.state.database, &[tool_status::PENDING_APPROVAL])
            .await?
            .is_empty();
        conversation
            .set_status(
                &self.state.database,
                if still_pending {
                    status::AWAITING_APPROVAL
                } else {
                    status::RUNNING
                },
            )
            .await?;

        Ok(if still_pending {
            StepResult::of(StepState::AwaitingApproval)
        } else {
            StepResult::proceed()
        })
    }

    async fn run_tool(
        &self,
        conversation: &Conversation,
        mut row: Message,
    ) -> Result<StepResult, anyhow::Error> {
        let tools_per_minute = self.state.settings.get().await?.neo.tools_per_minute.max(1);
        if self
            .state
            .cache
            .ratelimit(
                "neo::tools",
                tools_per_minute as u64 + 1,
                60,
                self.user.uuid.to_string(),
            )
            .await
            .is_err()
        {
            return Ok(StepResult::busy(5));
        }

        let meta = row.meta.clone().unwrap_or_else(|| json!({}));
        row.update_tool(
            self.state.database.write(),
            tool_status::EXECUTING,
            None,
            meta,
        )
        .await?;

        let started = std::time::Instant::now();
        let context = self.context(conversation);
        let name = row.tool_name.clone().unwrap_or_default();
        let arguments = row.arguments();

        let result = match tools::find(&name) {
            None => ToolResult::error(format!("There is no tool called {name}.")),
            Some(tool) if !tools::permitted(tool, &context, &arguments) => ToolResult::error(
                format!("The user does not have permission for {name} here."),
            ),
            Some(tool) => {
                let blocked = match (tool.scope(), self.server) {
                    (ToolScope::Server, Some(server)) if name != "server_status" => {
                        server_blocked(self.state, server).await
                    }
                    _ => None,
                };

                match blocked {
                    Some(reason) => {
                        ToolResult::error(format!("This cannot be done right now: {reason}."))
                    }
                    None => match tool.execute(&context, &arguments).await {
                        Ok(result) => result,
                        Err(ToolError::Invalid(message)) => ToolResult::error(message),
                        Err(ToolError::Failed(err)) => {
                            tracing::error!(tool = %name, conversation = %conversation.uuid, "neo tool failed: {err:#}");
                            ToolResult::error(
                                "The action failed on the server. Tell the user and suggest checking the console.",
                            )
                        }
                    },
                }
            }
        };

        let meta = row.meta_with(json!({
            "display": result.display,
            "ms": started.elapsed().as_millis() as i64,
        }));
        row.update_tool(
            self.state.database.write(),
            if result.ok {
                tool_status::DONE
            } else {
                tool_status::FAILED
            },
            Some(crate::redact::redact(&result.content)),
            meta,
        )
        .await?;

        sqlx::query("UPDATE neo_conversations SET last_activity = NOW() WHERE uuid = $1")
            .bind(conversation.uuid)
            .execute(self.state.database.write())
            .await?;

        Ok(match result.redirect {
            Some(url) => StepResult::redirect(url),
            None => StepResult::proceed(),
        })
    }

    async fn call_model(
        &self,
        conversation: &mut Conversation,
        deltas: Option<&DeltaSender>,
    ) -> Result<StepResult, anyhow::Error> {
        let (max_steps, context_tokens, always_confirm_destructive) = {
            let settings = self.state.settings.get().await?;
            (
                settings.neo.max_steps.max(1),
                settings.neo.context_tokens.max(4000) as usize,
                settings.neo.always_confirm_destructive,
            )
        };
        let preferences = Preferences::of(&self.state.database, self.user.uuid).await?;

        let final_answer = i32::from(conversation.step_count) >= max_steps;
        let context = self.context(conversation);
        let available = tools::available(&context);

        let system = super::prompt::system(
            self.state,
            &super::prompt::PromptFacts {
                user: self.user,
                server: self.server,
                permissions: self.permission_names(),
                auto_approve: preferences.auto_approve,
            },
        )
        .await;
        let rows: Vec<Message> = conversation
            .messages(&self.state.database)
            .await?
            .into_iter()
            .filter(|row| row.role != role::NOTICE)
            .collect();
        let messages = super::context::build(
            &self.state.database,
            system,
            &rows,
            context_tokens,
            final_answer,
        )
        .await;

        let conversation_key = conversation.uuid.to_string();
        let outcome = super::turn(
            self.state,
            self.user.uuid,
            Ask {
                conversation: &conversation_key,
                messages,
                tools: if final_answer {
                    Vec::new()
                } else {
                    tools::definitions(&available)
                },
                tool_choice: if final_answer { "none" } else { "auto" },
            },
            deltas,
        )
        .await;

        let answer = match outcome {
            Ok(answer) => answer,
            Err(TurnError::LimitReached {
                window, retry_at, ..
            }) => {
                let code = match window.as_str() {
                    "long" => "limited_long",
                    "model" => "limited_model",
                    "images" => "limited_images",
                    _ => "limited_short",
                };

                return self
                    .halt(
                        conversation,
                        status::LIMITED,
                        StepState::Limited,
                        code,
                        json!({"retry_at": retry_at, "window": window}),
                    )
                    .await;
            }
            Err(TurnError::ModelUnavailable {
                reason,
                label,
                access,
            }) => {
                let code = match reason.as_str() {
                    "model_not_allowed" => "model_not_allowed",
                    "model_unavailable" => "model_unavailable",
                    _ => "unavailable",
                };

                return self
                    .halt(
                        conversation,
                        status::ERROR,
                        StepState::Error,
                        code,
                        json!({"model": label, "access": access}),
                    )
                    .await;
            }
            Err(TurnError::NotLinked) => {
                return self
                    .halt(
                        conversation,
                        status::UNLINKED,
                        StepState::Unlinked,
                        "unlinked",
                        json!({}),
                    )
                    .await;
            }
            Err(TurnError::Busy { retry_after }) => return Ok(StepResult::busy(retry_after)),
            Err(TurnError::Unavailable { reason }) => {
                let reason: String = reason.chars().take(250).collect();
                sqlx::query("UPDATE neo_conversations SET last_error = $2 WHERE uuid = $1")
                    .bind(conversation.uuid)
                    .bind(&reason)
                    .execute(self.state.database.write())
                    .await?;

                return self
                    .halt(
                        conversation,
                        status::ERROR,
                        StepState::Error,
                        "unavailable",
                        json!({}),
                    )
                    .await;
            }
        };

        let calls: Vec<Value> = answer
            .tool_calls
            .iter()
            .filter(|call| call.is_object())
            .cloned()
            .collect();
        let content = answer
            .content
            .as_deref()
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .map(str::to_string);
        // what the model thought, for the chat to show folded away; never sent back to a model
        let thinking: Option<String> = Some(answer.thinking.trim())
            .filter(|thinking| !thinking.is_empty())
            .map(|thinking| thinking.chars().take(12_000).collect());
        let generated: Vec<Vec<u8>> = answer
            .images
            .iter()
            .filter_map(|image| {
                use base64::Engine;

                base64::engine::general_purpose::STANDARD
                    .decode(&image.data)
                    .ok()
                    .and_then(|bytes| super::images::jpeg(&bytes))
            })
            .collect();

        // the last step's calls are stored as skipped, so the turn ends with them
        let continues = !calls.is_empty() && !final_answer;

        let mut transaction = self.state.database.write().begin().await?;
        let mut meta = json!({
            "provider": answer.provider_label,
            "model": answer.provider_model,
            "finish_reason": answer.finish_reason,
            "tokens": answer.charged_tokens,
        });
        if let Some(thinking) = &thinking {
            meta["thinking"] = json!(thinking);
        }
        let assistant = conversation
            .append(
                &mut *transaction,
                role::ASSISTANT,
                NewMessage {
                    content,
                    tool_calls: (!calls.is_empty()).then(|| Value::Array(calls.clone())),
                    meta: Some(meta.clone()),
                    ..Default::default()
                },
            )
            .await?;
        if !generated.is_empty() {
            let uuids = super::images::store(&mut transaction, assistant.uuid, &generated).await?;
            meta["images"] = json!(uuids);
            sqlx::query("UPDATE neo_messages SET meta = $2 WHERE uuid = $1")
                .bind(assistant.uuid)
                .bind(&meta)
                .execute(&mut *transaction)
                .await?;
        }

        for call in &calls {
            let queued = queue(
                call,
                &available,
                final_answer,
                preferences.auto_approve,
                always_confirm_destructive,
            );
            conversation
                .append(&mut *transaction, role::TOOL, queued)
                .await?;
        }

        let provider: String = answer
            .provider_label
            .clone()
            .unwrap_or_default()
            .chars()
            .take(60)
            .collect();
        sqlx::query(
            "UPDATE neo_conversations SET status = $2, step_count = step_count + 1,
                tokens_used = tokens_used + $3, last_provider = $4, last_activity = NOW()
            WHERE uuid = $1",
        )
        .bind(conversation.uuid)
        .bind(if continues {
            status::RUNNING
        } else {
            status::IDLE
        })
        .bind(answer.charged_tokens.max(0))
        .bind(provider)
        .execute(&mut *transaction)
        .await?;

        if answer.finish_reason == "content_filter" {
            conversation
                .append(
                    &mut *transaction,
                    role::NOTICE,
                    NewMessage {
                        content: Some("The model refused to answer this.".into()),
                        meta: Some(json!({"kind": "error", "code": "refused"})),
                        ..Default::default()
                    },
                )
                .await?;
        }
        transaction.commit().await?;

        Ok(if continues {
            StepResult::proceed()
        } else {
            StepResult::done()
        })
    }

    /// Ends the loop with a notice the chat shows (and the person's next action clears).
    async fn halt(
        &self,
        conversation: &mut Conversation,
        conversation_status: &str,
        state: StepState,
        code: &'static str,
        params: Value,
    ) -> Result<StepResult, anyhow::Error> {
        let mut meta = json!({"kind": state, "code": code});
        if let (Some(meta), Some(params)) = (meta.as_object_mut(), params.as_object()) {
            meta.extend(params.clone());
        }

        conversation
            .append(
                self.state.database.write(),
                role::NOTICE,
                NewMessage {
                    content: Some(code.to_string()),
                    meta: Some(meta),
                    ..Default::default()
                },
            )
            .await?;
        conversation
            .set_status(&self.state.database, conversation_status)
            .await?;

        Ok(StepResult::with(state, code, params))
    }

    /// The person's permissions on the chat's server, for the system prompt.
    fn permission_names(&self) -> Vec<String> {
        let Some(server) = self.server else {
            return Vec::new();
        };
        if server.owner.uuid == self.user.uuid {
            return vec!["*".into()];
        }

        server
            .subuser_permissions
            .as_ref()
            .map(|permissions| permissions.iter().map(|p| p.to_string()).collect())
            .unwrap_or_default()
    }
}

/// One call from the model as a tool row, with the status the approval rules give it.
fn queue(
    call: &Value,
    available: &[&'static dyn tools::Tool],
    final_answer: bool,
    auto_approve: bool,
    always_confirm_destructive: bool,
) -> NewMessage {
    let name = call["function"]["name"].as_str().unwrap_or_default();
    let raw = call["function"]["arguments"].as_str().unwrap_or("{}");
    let arguments = match call["function"]["arguments"] {
        Value::Object(_) => Some(call["function"]["arguments"].clone()),
        _ => serde_json::from_str::<Value>(if raw.trim().is_empty() { "{}" } else { raw })
            .ok()
            .filter(Value::is_object),
    };
    let tool = available.iter().find(|tool| tool.name() == name);

    let mut status = tool_status::QUEUED;
    let mut content = None;
    let mut risk = ToolRisk::Read;
    let mut summary = name.to_string();

    if final_answer {
        status = tool_status::SKIPPED;
        content = Some("Not run: the step limit for this message was reached.".to_string());
    } else if let (Some(tool), Some(arguments)) = (tool, arguments.as_ref()) {
        risk = tool.risk(arguments);
        summary = tool.summarize(arguments);
        if risk.asks(auto_approve, always_confirm_destructive) {
            status = tool_status::PENDING_APPROVAL;
        }
    } else if tool.is_none() {
        status = tool_status::FAILED;
        content = Some(format!(
            "Tool {name} is not available here (unknown, or the user lacks the permission)."
        ));
    } else {
        status = tool_status::FAILED;
        content = Some(
            "The arguments were not valid JSON. Call the tool again with a JSON object."
                .to_string(),
        );
    }

    let arguments = arguments.unwrap_or_else(|| json!({}));
    let preview = preview(name, &arguments);

    NewMessage {
        content,
        tool_call_id: Some(
            call["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map_or_else(super::chat::new_call_id, |id| id.chars().take(64).collect()),
        ),
        tool_name: Some(name.chars().take(64).collect()),
        status: Some(status),
        meta: Some(json!({
            "arguments": arguments,
            "summary": summary,
            "risk": risk,
            "approval": if status == tool_status::QUEUED && risk != ToolRisk::Read { Some("auto") } else { None },
            "preview": preview,
        })),
        ..Default::default()
    }
}

/// What the approval card shows of a content-carrying call, redacted and short.
fn preview(tool: &str, arguments: &Value) -> Option<String> {
    let text = match tool {
        "write_file" => arguments["content"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        "edit_file" => format!(
            "- {}\n+ {}",
            arguments["old_text"].as_str().unwrap_or_default(),
            arguments["new_text"].as_str().unwrap_or_default()
        ),
        "send_command" => arguments["command"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        _ => return None,
    };

    let text = crate::redact::redact(&text);
    Some(if text.chars().count() > 1500 {
        format!("{}…", text.chars().take(1500).collect::<String>())
    } else {
        text
    })
}

/// A conversation's title from its first message.
fn title_of(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > 60 {
        format!("{}…", line.chars().take(59).collect::<String>())
    } else {
        line
    }
}

fn state_of(conversation: &Conversation) -> StepResult {
    StepResult::of(match conversation.status.as_str() {
        status::AWAITING_APPROVAL => StepState::AwaitingApproval,
        status::LIMITED => StepState::Limited,
        status::UNLINKED => StepState::Unlinked,
        status::ERROR => StepState::Error,
        _ => StepState::Done,
    })
}

/// Why the chat's server cannot be acted on right now, or `None` when it can (the rules are
/// `tools::console::blocked`, which `server_status` reports too).
pub async fn server_blocked(state: &crate::State, server: &Server) -> Option<&'static str> {
    tools::console::blocked(state, server).await
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(ToolRisk);

    #[async_trait::async_trait]
    impl tools::Tool for Fake {
        fn name(&self) -> &'static str {
            "write_file"
        }

        fn description(&self) -> std::borrow::Cow<'static, str> {
            "".into()
        }

        fn parameters(&self) -> Value {
            json!({"type": "object"})
        }

        fn risk(&self, _arguments: &Value) -> ToolRisk {
            self.0
        }

        fn summarize(&self, arguments: &Value) -> String {
            format!("Write {}", arguments["path"].as_str().unwrap_or("?"))
        }

        async fn execute(
            &self,
            _context: &ToolContext<'_>,
            _arguments: &Value,
        ) -> tools::ToolOutcome {
            Ok(ToolResult::ok(""))
        }
    }

    static WRITE: Fake = Fake(ToolRisk::Write);
    static READ: Fake = Fake(ToolRisk::Read);

    fn call(name: &str, arguments: &str) -> Value {
        json!({"id": "call_1", "type": "function", "function": {"name": name, "arguments": arguments}})
    }

    #[test]
    fn queues_calls_by_risk() {
        let available: Vec<&'static dyn tools::Tool> = vec![&WRITE];

        let row = queue(
            &call("write_file", r#"{"path":"/a","content":"x"}"#),
            &available,
            false,
            false,
            true,
        );
        assert_eq!(row.status, Some(tool_status::PENDING_APPROVAL));
        assert_eq!(row.meta.as_ref().unwrap()["summary"], "Write /a");
        assert_eq!(row.meta.as_ref().unwrap()["preview"], "x");

        let row = queue(&call("write_file", "{}"), &available, false, true, true);
        assert_eq!(row.status, Some(tool_status::QUEUED));
        assert_eq!(row.meta.as_ref().unwrap()["approval"], "auto");

        let reads: Vec<&'static dyn tools::Tool> = vec![&READ];
        let row = queue(&call("write_file", ""), &reads, false, false, true);
        assert_eq!(row.status, Some(tool_status::QUEUED));
        assert!(row.meta.as_ref().unwrap()["approval"].is_null());
    }

    #[test]
    fn refuses_unknown_tools_bad_arguments_and_steps_over_the_limit() {
        let available: Vec<&'static dyn tools::Tool> = vec![&WRITE];

        let row = queue(
            &call("delete_everything", "{}"),
            &available,
            false,
            false,
            true,
        );
        assert_eq!(row.status, Some(tool_status::FAILED));

        let row = queue(
            &call("write_file", "not json"),
            &available,
            false,
            false,
            true,
        );
        assert_eq!(row.status, Some(tool_status::FAILED));
        assert!(row.content.unwrap().contains("not valid JSON"));

        let row = queue(&call("write_file", "{}"), &available, true, false, true);
        assert_eq!(row.status, Some(tool_status::SKIPPED));
    }

    #[test]
    fn previews_and_titles() {
        assert_eq!(
            preview("edit_file", &json!({"old_text": "a", "new_text": "b"})).as_deref(),
            Some("- a\n+ b")
        );
        assert_eq!(preview("read_file", &json!({})), None);
        assert!(
            preview("send_command", &json!({"command": "x".repeat(2000)}))
                .unwrap()
                .ends_with('…')
        );
        assert_eq!(title_of("  fix\n my   server "), "fix my server");
        assert_eq!(title_of(&"y".repeat(80)).chars().count(), 60);
    }
}
