//! plugged.host: neo's provider chain, while the panel runs neo itself (and rux's AI, always on
//! the panel's chain). noprofit.space's failover, copied: providers are tried in slot order; one
//! that fails cools down (auth errors 10 minutes, rate limits and outages for their Retry-After
//! or the default, a request it found too large not at all) and is skipped meanwhile; a slot's
//! daily token cap is counted per day. When all fail, [`ChainError::Busy`] says how long until
//! one is back, unless none can be ([`ChainError::Unavailable`]).
//!
//! A chat turn runs on the model its user picked (one slot, no failover). Other turns fail over
//! across the models open to everyone. A turn with images tries the slots that see them first;
//! the others get a note in their place.
//!
//! Each caller has its own scope (`neo`, `rux`): cooldowns, daily counts and what a provider
//! does with tools are kept per scope.

use super::{
    chat,
    drivers::{self, Answer, Cooldowns, Failure},
    prompted::Prompted,
};
use crate::settings::neo::{AppSettingsNeo, NeoAccess, NeoProvider};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// How long a Gemini thought signature is kept for the next turn.
const SIGNATURE_TTL: u64 = 172_800;
/// How long a scope trusts what it found out about a provider passing tools on.
const TOOLS_PROBE_TTL: u64 = 21_600;
const AUTH_COOLDOWN: u64 = 600;
const MAX_COOLDOWN: u64 = 900;

pub struct Chain<'a> {
    /// `neo` or `rux`.
    pub scope: &'a str,
    /// Ask a provider once whether the tools of a request reach the model.
    pub probe_tools: bool,
}

/// A turn one provider answered.
#[derive(Debug, Clone)]
pub struct Turn {
    pub answer: Answer,
    pub provider: NeoProvider,
    pub duration_ms: i64,
    pub prompted_tools: bool,
}

#[derive(Debug, Clone)]
pub enum ChainError {
    /// No usable provider (or not the picked one).
    NotConfigured,
    /// Every provider failed for now; one is back in `retry_after` seconds.
    Busy {
        retry_after: u64,
        attempts: Vec<Value>,
    },
    /// Every provider failed and none comes back soon.
    Unavailable { attempts: Vec<Value> },
}

/// What the admin sees per slot.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SlotStatus {
    pub slot: u8,
    pub cooling_down_until: Option<i64>,
    pub tokens_today: i64,
    pub last_error: Option<LastError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LastError {
    pub at: i64,
    pub reason: String,
    pub status: Option<u16>,
}

fn cooldowns(settings: &AppSettingsNeo) -> Cooldowns {
    Cooldowns {
        default: settings.default_cooldown.max(1) as u64,
        auth: AUTH_COOLDOWN,
        max: MAX_COOLDOWN,
    }
}

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

impl Chain<'_> {
    /// Per scope and provider, and reset when its endpoint, model or key change.
    fn key(&self, provider: &NeoProvider, what: &str) -> String {
        let key_hash = hex::encode(Sha256::digest(
            provider.api_key.as_deref().unwrap_or_default().as_bytes(),
        ));
        let hash = hex::encode(Sha256::digest(
            format!("{}|{}|{key_hash}", provider.endpoint, provider.model).as_bytes(),
        ));

        format!("neo::{}::{}::{what}", self.scope, &hash[..16])
    }

    fn signature_key(&self, id: &str) -> String {
        format!("neo::{}::signature::{id}", self.scope)
    }

    pub async fn cooling_down_until(
        &self,
        state: &crate::State,
        provider: &NeoProvider,
    ) -> Option<i64> {
        let until: i64 = state
            .cache
            .get(&self.key(provider, "cooldown"))
            .await
            .ok()
            .flatten()?;

        (until > chrono::Utc::now().timestamp()).then_some(until)
    }

    pub async fn tokens_today(&self, state: &crate::State, provider: &NeoProvider) -> i64 {
        state
            .cache
            .get(&self.key(provider, &format!("tokens::{}", today())))
            .await
            .ok()
            .flatten()
            .unwrap_or(0)
    }

    pub async fn status(&self, state: &crate::State, provider: &NeoProvider) -> SlotStatus {
        SlotStatus {
            slot: provider.slot,
            cooling_down_until: self.cooling_down_until(state, provider).await,
            tokens_today: self.tokens_today(state, provider).await,
            last_error: state
                .cache
                .get(&self.key(provider, "last-error"))
                .await
                .ok()
                .flatten(),
        }
    }

    /// One model turn on the first provider that answers, or with `slot` on that slot only.
    pub async fn turn(
        &self,
        state: &crate::State,
        settings: &AppSettingsNeo,
        messages: &[Value],
        tools: &[Value],
        tool_choice: &str,
        slot: Option<u8>,
    ) -> Result<Turn, ChainError> {
        let mut providers: Vec<&NeoProvider> = settings
            .usable_providers()
            .filter(|provider| match slot {
                Some(slot) => provider.slot == slot,
                // models only some may pick never answer turns nobody picked them for
                None => provider.access == NeoAccess::Everyone,
            })
            .collect();
        if providers.is_empty() {
            return Err(ChainError::NotConfigured);
        }

        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_secs(settings.deadline.max(10) as u64);
        let images = chat::has_images(messages);
        if images {
            providers.sort_by_key(|provider| u8::from(!provider.sees_images()));
        }
        let mut transient = false;
        let mut attempts = Vec::new();

        for provider in &providers {
            let remaining = || {
                deadline
                    .saturating_duration_since(tokio::time::Instant::now())
                    .as_secs()
            };

            if self.cooling_down_until(state, provider).await.is_some() {
                transient = true;
                continue;
            }
            if provider.daily_token_cap > 0
                && self.tokens_today(state, provider).await >= provider.daily_token_cap
            {
                attempts.push(json!({"slot": provider.slot, "reason": "daily_token_cap"}));
                continue;
            }
            if remaining() < 5 {
                transient = true;
                break;
            }

            let slot_messages = if images && !provider.sees_images() {
                chat::without_images(messages)
            } else {
                messages.to_vec()
            };
            let started = std::time::Instant::now();
            let prompted = !tools.is_empty()
                && !self
                    .passes_tools(state, settings, provider, remaining())
                    .await;
            if prompted && remaining() < 5 {
                transient = true;
                break;
            }

            let timeout = (provider.timeout.max(5) as u64).min(remaining());
            let outcome = if prompted {
                let gateway_tools: Vec<String> = state
                    .cache
                    .get(&self.key(provider, "gateway-tools"))
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let prompted_messages = Prompted { agent: self.scope }.messages(
                    &slot_messages,
                    tools,
                    tool_choice,
                    &gateway_tools,
                );
                self.call(
                    state,
                    settings,
                    provider,
                    &prompted_messages,
                    &[],
                    tool_choice,
                    timeout,
                )
                .await
            } else {
                self.call(
                    state,
                    settings,
                    provider,
                    &slot_messages,
                    tools,
                    tool_choice,
                    timeout,
                )
                .await
            };

            match outcome {
                Ok(mut answer) => {
                    self.count_tokens(state, provider, answer.total_tokens())
                        .await;
                    if prompted {
                        let (content, calls) = Prompted { agent: self.scope }.answer(
                            answer.content.take(),
                            std::mem::take(&mut answer.tool_calls),
                            tools,
                            tool_choice,
                        );
                        answer.content = content;
                        if !calls.is_empty() {
                            answer.finish_reason = "tool_calls";
                        }
                        answer.tool_calls = calls;
                    }

                    return Ok(Turn {
                        answer,
                        provider: (*provider).clone(),
                        duration_ms: started.elapsed().as_millis() as i64,
                        prompted_tools: prompted,
                    });
                }
                Err(failure) => {
                    attempts.push(json!({
                        "slot": provider.slot,
                        "status": failure.status,
                        "reason": failure.reason,
                        "cooldown": failure.cooldown,
                    }));
                    transient = transient || failure.cooldown > 0;
                    self.fail(state, provider, &failure).await;
                }
            }
        }

        let mut soonest = None;
        for provider in &providers {
            if let Some(until) = self.cooling_down_until(state, provider).await {
                soonest = Some(soonest.map_or(until, |s: i64| s.min(until)));
            }
        }
        let now = chrono::Utc::now().timestamp();
        let soonest = soonest.unwrap_or(now + 2);

        if transient {
            Err(ChainError::Busy {
                retry_after: (soonest - now).max(1) as u64,
                attempts,
            })
        } else {
            Err(ChainError::Unavailable { attempts })
        }
    }

    /// One provider, once: no failover, cooldown, count or probe (the settings' Test).
    pub async fn probe(
        &self,
        state: &crate::State,
        settings: &AppSettingsNeo,
        provider: &NeoProvider,
        messages: &[Value],
    ) -> (Result<Answer, Failure>, i64) {
        let started = std::time::Instant::now();
        let outcome = drivers::call(
            &state.env,
            provider,
            messages,
            &[],
            "auto",
            (provider.timeout.max(5) as u64).min(30),
            &HashMap::new(),
            cooldowns(settings),
        )
        .await;

        (outcome, started.elapsed().as_millis() as i64)
    }

    #[allow(clippy::too_many_arguments)]
    async fn call(
        &self,
        state: &crate::State,
        settings: &AppSettingsNeo,
        provider: &NeoProvider,
        messages: &[Value],
        tools: &[Value],
        tool_choice: &str,
        timeout: u64,
    ) -> Result<Answer, Failure> {
        let mut signatures = HashMap::new();
        if provider.driver == crate::settings::neo::NeoDriver::Gemini {
            for message in messages {
                for call in message["tool_calls"].as_array().into_iter().flatten() {
                    if let Some(id) = call["id"].as_str()
                        && let Ok(Some(signature)) =
                            state.cache.get::<String>(&self.signature_key(id)).await
                    {
                        signatures.insert(id.to_string(), signature);
                    }
                }
            }
        }

        let answer = drivers::call(
            &state.env,
            provider,
            messages,
            tools,
            tool_choice,
            timeout,
            &signatures,
            cooldowns(settings),
        )
        .await?;

        for (id, signature) in &answer.signatures {
            if let Err(err) = state
                .cache
                .set(&self.signature_key(id), SIGNATURE_TTL, signature)
                .await
            {
                tracing::warn!("failed to keep a Gemini thought signature: {err:#}");
            }
        }

        Ok(answer)
    }

    /// Whether a provider hands the request's tools to the model. Some gateways drop them and put
    /// in their own; for those the tools are described in the prompt instead. Found out with a
    /// tiny call, then kept for 6 hours.
    async fn passes_tools(
        &self,
        state: &crate::State,
        settings: &AppSettingsNeo,
        provider: &NeoProvider,
        remaining: u64,
    ) -> bool {
        if !self.probe_tools || !settings.tools_probe {
            return true;
        }

        let key = self.key(provider, "passes-tools");
        if let Ok(Some(known)) = state.cache.get::<bool>(&key).await {
            return known;
        }

        let probe_name = format!("{}_probe", self.scope);
        let probe = json!({
            "type": "function",
            "function": {
                "name": probe_name,
                "description": "Confirms that tools work. Call it when asked to.",
                "parameters": {"type": "object", "properties": {}, "required": []},
            },
        });
        let messages = [
            json!({"role": "system", "content": format!("You are a test. Answer by calling the {probe_name} tool, nothing else. If you have no tool called {probe_name}, call no tool and reply with only the exact names of the tools you do have, comma separated.")}),
            json!({"role": "user", "content": format!("Call {probe_name} now.")}),
        ];
        let timeout = (provider.timeout.max(5) as u64)
            .min(30)
            .min(remaining.saturating_sub(5))
            .max(5);

        let Ok(answer) = self
            .call(
                state,
                settings,
                provider,
                &messages,
                &[probe],
                "auto",
                timeout,
            )
            .await
        else {
            // cannot tell: send the tools as usual, and the real call reports the failure
            return true;
        };
        self.count_tokens(state, provider, answer.total_tokens())
            .await;

        let passes = answer
            .tool_calls
            .iter()
            .any(|call| call["function"]["name"] == probe_name.as_str());
        let _ = state.cache.set(&key, TOOLS_PROBE_TTL, &passes).await;

        if !passes {
            let named = regex::Regex::new(r"^[A-Za-z][A-Za-z0-9_.:-]{1,63}$").unwrap();
            let gateway_tools: Vec<String> = answer
                .content
                .unwrap_or_default()
                .split([',', '\n'])
                .map(|name| {
                    name.trim_matches([' ', '\t', '.', '`', '*', '-'])
                        .to_string()
                })
                .filter(|name| {
                    *name != probe_name
                        && named.is_match(name)
                        && !name.chars().all(|c| c.is_ascii_digit())
                })
                .take(20)
                .collect();
            tracing::info!(
                slot = provider.slot,
                its_own_tools = ?gateway_tools,
                "[{}] AI provider drops the tools of a request: they go in the prompt",
                self.scope
            );
            let _ = state
                .cache
                .set(
                    &self.key(provider, "gateway-tools"),
                    TOOLS_PROBE_TTL,
                    &gateway_tools,
                )
                .await;
        }

        passes
    }

    async fn count_tokens(&self, state: &crate::State, provider: &NeoProvider, tokens: i64) {
        let key = self.key(provider, &format!("tokens::{}", today()));
        let used = self.tokens_today(state, provider).await;
        let _ = state
            .cache
            .set(&key, 172_800, &(used + tokens.max(0)))
            .await;
    }

    async fn fail(&self, state: &crate::State, provider: &NeoProvider, failure: &Failure) {
        tracing::warn!(
            slot = provider.slot,
            status = failure.status,
            cooldown = failure.cooldown,
            "[{}] AI provider failed: {}",
            self.scope,
            failure.reason
        );
        let now = chrono::Utc::now().timestamp();
        let _ = state
            .cache
            .set(
                &self.key(provider, "last-error"),
                86_400,
                &LastError {
                    at: now,
                    reason: failure.reason.clone(),
                    status: failure.status,
                },
            )
            .await;
        if failure.cooldown > 0 {
            let _ = state
                .cache
                .set(
                    &self.key(provider, "cooldown"),
                    failure.cooldown,
                    &(now + failure.cooldown as i64),
                )
                .await;
        }
    }
}
