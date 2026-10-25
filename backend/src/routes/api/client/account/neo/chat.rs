// plugged.host: neo's chat (shared/src/neo/agent.rs, docs/features/neo.md). The same routes serve
// the account's chat (`/account/neo`) and each server's chat (`/servers/{server}/neo`): a handler
// knows its place by whether a server was loaded for the request.

use super::State;
use axum::extract::DefaultBodyLimit;
use shared::{
    models::{
        server::Server,
        user::{PermissionManager, User},
    },
    neo::{agent::Agent, conversations::Conversation},
    response::ApiResponse,
};
use utoipa_axum::{
    router::{OpenApiRouter, UtoipaMethodRouterExt},
    routes,
};

/// neo.chat on the account, and neo.access on a server too.
fn may_chat(permissions: &PermissionManager, server: Option<&Server>) -> Result<(), ApiResponse> {
    permissions.has_user_permission("neo.chat")?;
    if server.is_some() {
        permissions.has_server_permission("neo.access")?;
    }

    Ok(())
}

fn agent<'a>(
    state: &'a shared::State,
    user: &'a User,
    server: Option<&'a Server>,
    permissions: &'a PermissionManager,
    ip: std::net::IpAddr,
) -> Agent<'a> {
    Agent {
        state,
        user,
        server,
        permissions,
        ip,
    }
}

#[derive(serde::Deserialize)]
struct ConversationPath {
    conversation: uuid::Uuid,
}

/// One of the person's chats in this place.
async fn own(
    state: &shared::State,
    user: &User,
    server: Option<&Server>,
    conversation: uuid::Uuid,
) -> Result<Conversation, ApiResponse> {
    match Conversation::own(
        &state.database,
        conversation,
        user.uuid,
        server.map(|server| server.uuid),
    )
    .await?
    {
        Some(conversation) => Ok(conversation),
        None => Err(ApiResponse::error("conversation not found")
            .with_status(axum::http::StatusCode::NOT_FOUND)),
    }
}

/// What every chat action answers: where the loop stands and the chat as it is now.
#[derive(utoipa::ToSchema, serde::Serialize)]
struct StepResponse {
    result: shared::neo::agent::StepResult,
    conversation: shared::neo::conversations::ApiNeoConversation,
}

async fn step_response(
    state: &shared::State,
    result: shared::neo::agent::StepResult,
    conversation: &Conversation,
) -> Result<ApiResponse, ApiResponse> {
    let conversation = match conversation.refresh(&state.database).await {
        Ok(conversation) => conversation,
        // neo deleted the chat's own server (`delete_server`) and the chat went with it: answer
        // with the last copy, so the person still gets the redirect.
        Err(sqlx::Error::RowNotFound) => conversation.clone(),
        Err(err) => Err(err)?,
    }
    .into_api(&state.database)
    .await?;

    Ok(ApiResponse::new_serialized(StepResponse {
        result,
        conversation,
    }))
}

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetIp, GetState,
        models::{
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        neo::{
            agent::MAX_MESSAGE_CHARS,
            conversations::{ApiNeoConversation, Preferences},
            images::MAX_IMAGES,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct ApiNeo {
        /// Why the person cannot chat here right now: not_yours, disabled, turned_off, consent,
        /// no_access.
        blocked: Option<&'static str>,
        preferences: Preferences,
        conversation: ApiNeoConversation,
        /// Where neo sends people for human help; empty: no link.
        support_url: String,
        legal_email: String,
        privacy_email: String,
        /// Who processes the messages, shown before the person agrees.
        provider_disclosure: String,
        max_message_chars: usize,
        max_images: usize,
        /// Days a chat is kept after its last message (the settings' retention, at least 1).
        retention_days: i32,
        /// noprofit.space runs neo, so its usage windows are noprofit.space's (shared with its
        /// other services).
        on_noprofit: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        neo: ApiNeo,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        ip: GetIp,
        permissions: GetPermissionManager,
        user: GetUser,
        server: Option<GetServer>,
    ) -> ApiResponseResult {
        let server = server.as_deref();
        super::may_chat(&permissions, server)?;

        let agent = super::agent(&state, &user, server, &permissions, *ip);
        let conversation = agent.conversation().await?;
        let blocked = agent.blocked(&conversation).await?;
        let preferences = Preferences::of(&state.database, user.uuid).await?;

        let settings = state.settings.get().await?;
        let support_url = settings.neo.support_url.to_string();
        let legal_email = settings.neo.legal_email.to_string();
        let privacy_email = settings.neo.privacy_email.to_string();
        let provider_disclosure = settings.neo.provider_disclosure.to_string();
        let retention_days = settings.neo.retention_days.max(1);
        let on_noprofit = shared::neo::on_noprofit(&settings).is_some();
        drop(settings);

        ApiResponse::new_serialized(Response {
            neo: ApiNeo {
                blocked,
                preferences,
                conversation: conversation.into_api(&state.database).await?,
                support_url,
                legal_email,
                privacy_email,
                provider_disclosure,
                max_message_chars: MAX_MESSAGE_CHARS,
                max_images: MAX_IMAGES,
                retention_days,
                on_noprofit,
            },
        })
        .ok()
    }
}

mod messages {
    pub mod post {
        use axum::http::StatusCode;
        use garde::Validate;
        use serde::Deserialize;
        use shared::{
            ApiError, GetIp, GetState,
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            neo::agent::{MAX_MESSAGE_CHARS, Seed},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        /// The largest image accepted, base64 included.
        const MAX_IMAGE_CHARS: usize = 15 * 1024 * 1024;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct PayloadSeed {
            /// A read tool the turn starts with, e.g. `read_mclogs` from the log check.
            #[garde(length(chars, max = 64))]
            tool: String,
            #[garde(skip)]
            #[schema(value_type = Object)]
            arguments: serde_json::Value,
        }

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            #[garde(length(chars, max = MAX_MESSAGE_CHARS))]
            text: String,
            /// Images as data URLs (PNG, JPEG, WebP or GIF).
            #[garde(length(max = shared::neo::images::MAX_IMAGES), inner(length(max = MAX_IMAGE_CHARS)))]
            #[serde(default)]
            images: Vec<String>,
            #[garde(dive)]
            seed: Option<PayloadSeed>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(super::super::StepResponse)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            permissions: GetPermissionManager,
            user: GetUser,
            server: Option<GetServer>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }
            let server = server.as_deref();
            super::super::may_chat(&permissions, server)?;

            let mut images = Vec::with_capacity(data.images.len());
            for image in &data.images {
                match decode(image).and_then(|bytes| shared::neo::images::jpeg(&bytes)) {
                    Some(jpeg) => images.push(jpeg),
                    None => {
                        return ApiResponse::error("an image could not be read")
                            .with_status(StatusCode::BAD_REQUEST)
                            .ok();
                    }
                }
            }

            let seed = match data.seed {
                Some(seed) => match shared::neo::tools::find(&seed.tool) {
                    Some(tool) => Some(Seed {
                        tool: tool.name(),
                        arguments: seed.arguments,
                    }),
                    None => {
                        return ApiResponse::error("unknown tool")
                            .with_status(StatusCode::BAD_REQUEST)
                            .ok();
                    }
                },
                None => None,
            };

            let agent = super::super::agent(&state, &user, server, &permissions, *ip);
            let mut conversation = agent.conversation().await?;
            let result = agent
                .send(&mut conversation, &data.text, images, seed)
                .await?;

            super::super::step_response(&state, result, &conversation)
                .await?
                .ok()
        }

        /// The bytes of a data URL (or of bare base64).
        fn decode(image: &str) -> Option<Vec<u8>> {
            use base64::Engine;

            let data = match image.strip_prefix("data:") {
                Some(rest) => rest.split_once(";base64,")?.1,
                None => image,
            };

            base64::engine::general_purpose::STANDARD
                .decode(data.trim())
                .ok()
        }
    }
}

mod step {
    pub mod post {
        use axum::body::Bytes;
        use serde_json::{Value, json};
        use shared::{
            ApiError, GetIp, GetState,
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            neo::{
                Delta,
                agent::{Agent, StepResult},
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use tokio::sync::mpsc::UnboundedSender;

        /// A line every so often while nothing else is said, so proxies keep the stream open.
        const PING: std::time::Duration = std::time::Duration::from_secs(15);

        /// One unit of work (a tool or a model call), answered as NDJSON: `text`, `thinking` and
        /// `reset` lines while the model writes, `ping` lines while nothing happens, then one
        /// `result` line with `result` (a step result) and `conversation`. The step finishes even
        /// when the browser goes away.
        #[utoipa::path(post, path = "/", responses(
            (status = OK, content_type = "application/x-ndjson", body = String),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            permissions: GetPermissionManager,
            user: GetUser,
            server: Option<GetServer>,
        ) -> ApiResponseResult {
            super::super::may_chat(&permissions, server.as_deref())?;

            let state = state.0;
            let ip = ip.0;
            let permissions = permissions.0;
            let user = user.0;
            let server = server.map(|server| server.0);
            let (lines, body) = tokio::sync::mpsc::unbounded_channel::<Bytes>();

            tokio::spawn(async move {
                let agent = super::super::agent(&state, &user, server.as_ref(), &permissions, ip);
                run(&agent, &lines).await;
            });

            let stream = futures_util::stream::unfold(body, |mut body| async move {
                body.recv()
                    .await
                    .map(|line| (Ok::<_, std::convert::Infallible>(line), body))
            });

            ApiResponse::new(axum::body::Body::from_stream(stream))
                .with_header("Content-Type", "application/x-ndjson")
                .with_header("Cache-Control", "no-store")
                .with_header("X-Accel-Buffering", "no")
                .ok()
        }

        fn send(lines: &UnboundedSender<Bytes>, value: &Value) {
            let mut line = serde_json::to_vec(value).unwrap_or_default();
            line.push(b'\n');
            // a closed stream is a browser that went away; the step goes on regardless
            let _ = lines.send(line.into());
        }

        fn delta(delta: Delta) -> Value {
            match delta {
                Delta::Text(text) => json!({"type": "text", "text": text}),
                Delta::Thinking(text) => json!({"type": "thinking", "text": text}),
                Delta::Reset => json!({"type": "reset"}),
            }
        }

        async fn run(agent: &Agent<'_>, lines: &UnboundedSender<Bytes>) {
            let (deltas, mut incoming) = tokio::sync::mpsc::unbounded_channel();
            let work = async {
                let mut conversation = agent.conversation().await?;
                let result = agent.step(&mut conversation, Some(&deltas)).await?;

                Ok::<_, anyhow::Error>((result, conversation))
            };
            tokio::pin!(work);

            let mut ping = tokio::time::interval(PING);
            ping.tick().await;
            let finished = loop {
                tokio::select! {
                    finished = &mut work => break finished,
                    Some(item) = incoming.recv() => send(lines, &delta(item)),
                    _ = ping.tick() => send(lines, &json!({"type": "ping"})),
                }
            };
            while let Ok(item) = incoming.try_recv() {
                send(lines, &delta(item));
            }

            let (result, conversation) = match finished {
                Ok((result, conversation)) => {
                    let view = match conversation.refresh(&agent.state.database).await {
                        Ok(conversation) => conversation.into_api(&agent.state.database).await,
                        Err(err) => Err(err),
                    };
                    match view {
                        Ok(view) => (result, Some(view)),
                        Err(err) => {
                            tracing::error!(conversation = %conversation.uuid, "neo step view failed: {err:#}");
                            (result, None)
                        }
                    }
                }
                Err(err) => {
                    tracing::error!(user = %agent.user.uuid, "neo step failed: {err:#}");
                    (StepResult::error("unavailable"), None)
                }
            };

            send(
                lines,
                &json!({"type": "result", "result": result, "conversation": conversation}),
            );
        }
    }
}

/// approve, deny, cancel and retry: small actions on the open chat.
mod decide {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        models::{
            server::Server,
            user::{PermissionManager, User},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// One waiting call; none approves all of them (deny needs one).
        #[garde(skip)]
        #[serde(default)]
        message: Option<uuid::Uuid>,
    }

    enum Action {
        Approve,
        Deny,
        Cancel,
        Retry,
    }

    async fn act(
        state: &shared::State,
        ip: std::net::IpAddr,
        permissions: &PermissionManager,
        user: &User,
        server: Option<&Server>,
        action: Action,
        message: Option<uuid::Uuid>,
    ) -> ApiResponseResult {
        super::may_chat(permissions, server)?;

        let agent = super::agent(state, user, server, permissions, ip);
        let mut conversation = agent.conversation().await?;
        let result = match action {
            Action::Approve => agent.approve(&mut conversation, message).await?,
            Action::Deny => match message {
                Some(message) => agent.deny(&mut conversation, message).await?,
                None => {
                    return ApiResponse::error("message is required")
                        .with_status(StatusCode::BAD_REQUEST)
                        .ok();
                }
            },
            Action::Cancel => agent.cancel(&mut conversation).await?,
            Action::Retry => agent.retry(&mut conversation).await?,
        };

        super::step_response(state, result, &conversation)
            .await?
            .ok()
    }

    pub mod approve {
        use shared::{
            ApiError, GetIp, GetState,
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            response::ApiResponseResult,
        };

        /// Approves one waiting call, or all of them.
        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(super::super::StepResponse)),
            (status = UNAUTHORIZED, body = ApiError),
        ), request_body = inline(super::Payload))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            permissions: GetPermissionManager,
            user: GetUser,
            server: Option<GetServer>,
            shared::Payload(data): shared::Payload<super::Payload>,
        ) -> ApiResponseResult {
            super::act(
                &state,
                *ip,
                &permissions,
                &user,
                server.as_deref(),
                super::Action::Approve,
                data.message,
            )
            .await
        }
    }

    pub mod deny {
        use shared::{
            ApiError, GetIp, GetState,
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            response::ApiResponseResult,
        };

        /// Declines one waiting call.
        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(super::super::StepResponse)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ), request_body = inline(super::Payload))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            permissions: GetPermissionManager,
            user: GetUser,
            server: Option<GetServer>,
            shared::Payload(data): shared::Payload<super::Payload>,
        ) -> ApiResponseResult {
            super::act(
                &state,
                *ip,
                &permissions,
                &user,
                server.as_deref(),
                super::Action::Deny,
                data.message,
            )
            .await
        }
    }

    pub mod cancel {
        use shared::{
            ApiError, GetIp, GetState,
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            response::ApiResponseResult,
        };

        /// Stops the current request: nothing still waiting runs.
        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(super::super::StepResponse)),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            permissions: GetPermissionManager,
            user: GetUser,
            server: Option<GetServer>,
        ) -> ApiResponseResult {
            super::act(
                &state,
                *ip,
                &permissions,
                &user,
                server.as_deref(),
                super::Action::Cancel,
                None,
            )
            .await
        }
    }

    pub mod retry {
        use shared::{
            ApiError, GetIp, GetState,
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            response::ApiResponseResult,
        };

        /// Tries again after an outage or once a limit has reset.
        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(super::super::StepResponse)),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            permissions: GetPermissionManager,
            user: GetUser,
            server: Option<GetServer>,
        ) -> ApiResponseResult {
            super::act(
                &state,
                *ip,
                &permissions,
                &user,
                server.as_deref(),
                super::Action::Retry,
                None,
            )
            .await
        }
    }
}

mod new {
    pub mod post {
        use serde::Serialize;
        use shared::{
            ApiError, GetIp, GetState,
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            neo::conversations::{ApiNeoConversation, status},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            conversation: ApiNeoConversation,
        }

        /// Puts the open chat into the history and starts an empty one.
        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = CONFLICT, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            permissions: GetPermissionManager,
            user: GetUser,
            server: Option<GetServer>,
        ) -> ApiResponseResult {
            let server = server.as_deref();
            super::super::may_chat(&permissions, server)?;

            let agent = super::super::agent(&state, &user, server, &permissions, *ip);
            let current = agent.conversation().await?;
            if current.status == status::RUNNING {
                return ApiResponse::error("neo is still answering")
                    .with_status(axum::http::StatusCode::CONFLICT)
                    .ok();
            }
            if current.has_messages(&state.database).await? {
                current.archive(&state.database).await?;
            }

            ApiResponse::new_serialized(Response {
                conversation: agent
                    .conversation()
                    .await?
                    .into_api(&state.database)
                    .await?,
            })
            .ok()
        }
    }
}

mod conversations {
    use super::State;
    use utoipa_axum::{router::OpenApiRouter, routes};

    mod get {
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            neo::conversations::{ApiNeoHistoryEntry, Conversation},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            conversations: Vec<ApiNeoHistoryEntry>,
        }

        /// This place's chats, newest first (the 50 latest with a message).
        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            server: Option<GetServer>,
        ) -> ApiResponseResult {
            let server = server.as_deref();
            super::super::may_chat(&permissions, server)?;

            let conversations =
                Conversation::history(&state.database, user.uuid, server.map(|server| server.uuid))
                    .await?
                    .into_iter()
                    .map(|(conversation, user_messages)| {
                        conversation.into_history_entry(user_messages)
                    })
                    .collect();

            ApiResponse::new_serialized(Response { conversations }).ok()
        }
    }

    mod conversation {
        pub mod get {
            use axum::extract::Path;
            use serde::Serialize;
            use shared::{
                ApiError, GetState,
                models::{
                    server::GetServer,
                    user::{GetPermissionManager, GetUser},
                },
                neo::conversations::ApiNeoConversation,
                response::{ApiResponse, ApiResponseResult},
            };
            use utoipa::ToSchema;

            #[derive(ToSchema, Serialize)]
            struct Response {
                conversation: ApiNeoConversation,
            }

            #[utoipa::path(get, path = "/", responses(
                (status = OK, body = inline(Response)),
                (status = NOT_FOUND, body = ApiError),
            ), params(
                (
                    "conversation" = uuid::Uuid,
                    description = "The conversation ID",
                    example = "123e4567-e89b-12d3-a456-426614174000",
                ),
            ))]
            pub async fn route(
                state: GetState,
                permissions: GetPermissionManager,
                user: GetUser,
                server: Option<GetServer>,
                Path(path): Path<super::super::super::ConversationPath>,
            ) -> ApiResponseResult {
                let server = server.as_deref();
                super::super::super::may_chat(&permissions, server)?;

                let conversation =
                    super::super::super::own(&state, &user, server, path.conversation).await?;

                ApiResponse::new_serialized(Response {
                    conversation: conversation.into_api(&state.database).await?,
                })
                .ok()
            }
        }

        pub mod delete {
            use axum::extract::Path;
            use serde::Serialize;
            use shared::{
                ApiError, GetState,
                models::{
                    server::GetServer,
                    user::{GetPermissionManager, GetUser},
                },
                neo::conversations::status,
                response::{ApiResponse, ApiResponseResult},
            };
            use utoipa::ToSchema;

            #[derive(ToSchema, Serialize)]
            struct Response {}

            #[utoipa::path(delete, path = "/", responses(
                (status = OK, body = inline(Response)),
                (status = NOT_FOUND, body = ApiError),
                (status = CONFLICT, body = ApiError),
            ), params(
                (
                    "conversation" = uuid::Uuid,
                    description = "The conversation ID",
                    example = "123e4567-e89b-12d3-a456-426614174000",
                ),
            ))]
            pub async fn route(
                state: GetState,
                permissions: GetPermissionManager,
                user: GetUser,
                server: Option<GetServer>,
                Path(path): Path<super::super::super::ConversationPath>,
            ) -> ApiResponseResult {
                let server = server.as_deref();
                super::super::super::may_chat(&permissions, server)?;

                let conversation =
                    super::super::super::own(&state, &user, server, path.conversation).await?;
                if conversation.status == status::RUNNING {
                    return ApiResponse::error("neo is still answering")
                        .with_status(axum::http::StatusCode::CONFLICT)
                        .ok();
                }
                conversation.delete(&state.database).await?;

                ApiResponse::new_serialized(Response {}).ok()
            }
        }

        pub mod open {
            pub mod post {
                use axum::extract::Path;
                use serde::Serialize;
                use shared::{
                    ApiError, GetIp, GetState,
                    models::{
                        server::GetServer,
                        user::{GetPermissionManager, GetUser},
                    },
                    neo::conversations::{ApiNeoConversation, status},
                    response::{ApiResponse, ApiResponseResult},
                };
                use utoipa::ToSchema;

                #[derive(ToSchema, Serialize)]
                struct Response {
                    conversation: ApiNeoConversation,
                }

                /// Continues an earlier chat: it becomes the open one, the open one goes into
                /// the history (or away, when it is empty).
                #[utoipa::path(post, path = "/", responses(
                    (status = OK, body = inline(Response)),
                    (status = NOT_FOUND, body = ApiError),
                    (status = CONFLICT, body = ApiError),
                ), params(
                    (
                        "conversation" = uuid::Uuid,
                        description = "The conversation ID",
                        example = "123e4567-e89b-12d3-a456-426614174000",
                    ),
                ))]
                pub async fn route(
                    state: GetState,
                    ip: GetIp,
                    permissions: GetPermissionManager,
                    user: GetUser,
                    server: Option<GetServer>,
                    Path(path): Path<super::super::super::super::ConversationPath>,
                ) -> ApiResponseResult {
                    let server = server.as_deref();
                    super::super::super::super::may_chat(&permissions, server)?;

                    let target =
                        super::super::super::super::own(&state, &user, server, path.conversation)
                            .await?;
                    let agent =
                        super::super::super::super::agent(&state, &user, server, &permissions, *ip);
                    let current = agent.conversation().await?;

                    if current.uuid != target.uuid {
                        if current.status == status::RUNNING {
                            return ApiResponse::error("neo is still answering")
                                .with_status(axum::http::StatusCode::CONFLICT)
                                .ok();
                        }
                        if current.has_messages(&state.database).await? {
                            current.archive(&state.database).await?;
                        } else {
                            current.delete(&state.database).await?;
                        }
                        target.unarchive(&state.database).await?;
                    }

                    ApiResponse::new_serialized(Response {
                        conversation: target
                            .refresh(&state.database)
                            .await?
                            .into_api(&state.database)
                            .await?,
                    })
                    .ok()
                }
            }
        }
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(get::route))
            .nest(
                "/{conversation}",
                OpenApiRouter::new()
                    .routes(routes!(conversation::get::route))
                    .routes(routes!(conversation::delete::route))
                    .nest(
                        "/open",
                        OpenApiRouter::new()
                            .routes(routes!(conversation::open::post::route))
                            .with_state(state.clone()),
                    )
                    .with_state(state.clone()),
            )
            .with_state(state.clone())
    }
}

/// The chat's routes, for the account's chat and for a server's.
pub fn router(state: &State) -> OpenApiRouter<State> {
    fn single(state: &State, router: OpenApiRouter<State>) -> OpenApiRouter<State> {
        router.with_state(state.clone())
    }

    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest(
            "/messages",
            single(
                state,
                OpenApiRouter::new().routes(
                    routes!(messages::post::route).layer(DefaultBodyLimit::max(64 * 1024 * 1024)),
                ),
            ),
        )
        .nest(
            "/step",
            single(
                state,
                OpenApiRouter::new().routes(routes!(step::post::route)),
            ),
        )
        .nest(
            "/approve",
            single(
                state,
                OpenApiRouter::new().routes(routes!(decide::approve::route)),
            ),
        )
        .nest(
            "/deny",
            single(
                state,
                OpenApiRouter::new().routes(routes!(decide::deny::route)),
            ),
        )
        .nest(
            "/cancel",
            single(
                state,
                OpenApiRouter::new().routes(routes!(decide::cancel::route)),
            ),
        )
        .nest(
            "/retry",
            single(
                state,
                OpenApiRouter::new().routes(routes!(decide::retry::route)),
            ),
        )
        .nest(
            "/new",
            single(
                state,
                OpenApiRouter::new().routes(routes!(new::post::route)),
            ),
        )
        .nest("/conversations", conversations::router(state))
        .with_state(state.clone())
}
