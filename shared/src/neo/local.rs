//! plugged.host: neo run by the panel, what noprofit.space's neo gateway does, here. A turn takes
//! the subject's turn lock and a concurrency slot, checks its windows, calls the picked model
//! ([`super::chain`], scope `neo`) and charges the windows. Every model is metered in the shared
//! windows (the gateway's limits mode `all`), and answers are not streamed: a turn hands its text
//! on in one piece.

use super::{
    Ask, Delta, DeltaSender, Model, TurnAnswer, TurnError, UsageInfo,
    chain::{Chain, ChainError},
    usage::{self, Event, Subject},
};
use crate::settings::neo::{AppSettingsNeo, NeoAccess, NeoPlan};

/// The service the panel's own turns are recorded under.
pub const SERVICE: &str = "panel";

pub async fn turn(
    state: &crate::State,
    neo: &AppSettingsNeo,
    user_uuid: uuid::Uuid,
    plan: NeoPlan,
    usage_percent: i64,
    ask: Ask<'_>,
    deltas: Option<&DeltaSender>,
) -> Result<TurnAnswer, TurnError> {
    let subject = Subject::user(user_uuid, usage_percent);
    let picked = super::picked_model(&state.database, user_uuid).await;
    let slot = slot_for(neo, picked.as_deref(), plan)?;

    let answer = run(state, neo, &subject, SERVICE, ask, slot, plan).await?;
    if let Some(deltas) = deltas
        && let Some(content) = answer.content.as_ref().filter(|c| !c.is_empty())
    {
        let _ = deltas.send(Delta::Text(content.clone()));
    }

    Ok(answer)
}

pub async fn usage(
    state: &crate::State,
    neo: &AppSettingsNeo,
    user_uuid: uuid::Uuid,
    plan: NeoPlan,
    usage_percent: i64,
) -> Option<UsageInfo> {
    let subject = Subject::user(user_uuid, usage_percent);
    let limits = match usage::snapshot(&state.database, neo, &subject).await {
        Ok(limits) => limits,
        Err(err) => {
            tracing::warn!("failed to read neo's windows: {err:#}");
            return None;
        }
    };
    let (models, default_model) = catalog(neo, plan);

    Some(UsageInfo {
        linked: true,
        limits: Some(limits),
        mode: "all".into(),
        models,
        default_model,
    })
}

/// One metered turn on `slot`.
pub async fn run(
    state: &crate::State,
    neo: &AppSettingsNeo,
    subject: &Subject,
    service: &str,
    ask: Ask<'_>,
    slot: u8,
    plan: NeoPlan,
) -> Result<TurnAnswer, TurnError> {
    if !neo.gateway_enabled {
        return Err(TurnError::Unavailable {
            reason: "gateway_disabled".into(),
        });
    }

    let lock_seconds = neo.deadline.max(10) as u64 + 15;
    let Ok(_turn_lock) = state
        .cache
        .lock(
            format!("neo::turn::{}", subject.lock_key()),
            Some(lock_seconds),
            Some(3000),
        )
        .await
    else {
        return Err(TurnError::Busy { retry_after: 2 });
    };

    let limits = usage::snapshot(&state.database, neo, subject)
        .await
        .map_err(|err| TurnError::Unavailable {
            reason: format!("usage: {err}"),
        })?;
    if let Some(window) = usage::blocking_window(&limits) {
        return Err(TurnError::LimitReached {
            retry_at: limits.get(&window).and_then(|w| w.resets_at.clone()),
            limits: serde_json::to_value(&limits).unwrap_or_default(),
            window,
        });
    }

    let mut concurrency = None;
    for i in 1..=neo.max_concurrent.max(1) {
        if let Ok(lock) = state
            .cache
            .lock(
                format!("neo::concurrency::{i}"),
                Some(lock_seconds),
                Some(1),
            )
            .await
        {
            concurrency = Some(lock);
            break;
        }
    }
    let Some(concurrency) = concurrency else {
        return Err(TurnError::Busy { retry_after: 3 });
    };

    let mut event = Event {
        service: service.to_string(),
        conversation: Some(ask.conversation.to_string()),
        ..Default::default()
    };

    let outcome = Chain {
        scope: "neo",
        probe_tools: true,
    }
    .turn(
        state,
        neo,
        &ask.messages,
        &ask.tools,
        ask.tool_choice,
        Some(slot),
    )
    .await;
    drop(concurrency);

    let turn = match outcome {
        Ok(turn) => turn,
        Err(error) => {
            let (attempts, failure) = match error {
                ChainError::NotConfigured => (
                    Vec::new(),
                    TurnError::ModelUnavailable {
                        reason: "model_unavailable".into(),
                        label: None,
                        access: None,
                    },
                ),
                ChainError::Busy {
                    retry_after,
                    attempts,
                } if retry_after <= neo.max_busy_wait.max(1) as u64 => {
                    (attempts, TurnError::Busy { retry_after })
                }
                ChainError::Busy { attempts, .. } | ChainError::Unavailable { attempts } => (
                    attempts,
                    TurnError::Unavailable {
                        reason: "providers_unavailable".into(),
                    },
                ),
            };
            event.attempts = Some(serde_json::Value::Array(attempts));
            if let Err(err) = usage::record(&state.database, Some(subject), "failed", &event).await
            {
                tracing::warn!("failed to record a failed neo turn: {err:#}");
            }

            return Err(failure);
        }
    };

    let charged = turn.answer.total_tokens() * turn.provider.multiplier();
    event.provider_slot = Some(turn.provider.slot as i16);
    event.provider_label = Some(turn.provider.label.to_string());
    event.driver = Some(turn.provider.driver.as_str().to_string());
    event.model = Some(turn.provider.model.to_string());
    event.prompt_tokens = turn.answer.prompt_tokens;
    event.completion_tokens = turn.answer.completion_tokens;
    event.finish_reason = Some(turn.answer.finish_reason.to_string());
    event.duration_ms = turn.duration_ms;
    if let Err(err) = usage::charge(&state.database, neo, subject, charged, &event).await {
        tracing::error!("failed to charge a neo turn: {err:#}");
    }

    let limits = usage::snapshot(&state.database, neo, subject).await.ok();
    let (models, default_model) = catalog(neo, plan);

    Ok(TurnAnswer {
        content: turn.answer.content,
        tool_calls: turn.answer.tool_calls,
        thinking: String::new(),
        images: Vec::new(),
        finish_reason: turn.answer.finish_reason.to_string(),
        provider_label: Some(turn.provider.label.to_string()),
        provider_model: Some(turn.provider.model.to_string()),
        charged_tokens: charged,
        usage: UsageInfo {
            linked: !subject.is_unlinked_discord(),
            limits,
            mode: "all".into(),
            models,
            default_model,
        },
    })
}

/// The slot a turn runs on: the picked one while it is usable and open to `plan`, else the first
/// usable one open to `plan`, preferring ones open to everyone.
pub fn slot_for(
    neo: &AppSettingsNeo,
    picked: Option<&str>,
    plan: NeoPlan,
) -> Result<u8, TurnError> {
    let usable: Vec<_> = neo.usable_providers().collect();

    if let Some(picked) = picked.and_then(|picked| picked.parse::<u8>().ok()) {
        return match usable.iter().find(|provider| provider.slot == picked) {
            Some(provider) if provider.access.allows(plan) => Ok(provider.slot),
            Some(provider) => Err(TurnError::ModelUnavailable {
                reason: "model_not_allowed".into(),
                label: Some(provider.label.to_string()),
                access: Some(provider.access.as_str().into()),
            }),
            None => Err(TurnError::ModelUnavailable {
                reason: "model_unavailable".into(),
                label: None,
                access: None,
            }),
        };
    }

    let open: Vec<_> = usable
        .iter()
        .filter(|provider| provider.access.allows(plan))
        .collect();

    open.iter()
        .find(|provider| provider.access == NeoAccess::Everyone)
        .or_else(|| open.first())
        .map(|provider| provider.slot)
        .ok_or(TurnError::ModelUnavailable {
            reason: "no_model".into(),
            label: None,
            access: None,
        })
}

/// The model picker, as the gateway answers it.
pub fn catalog(neo: &AppSettingsNeo, plan: NeoPlan) -> (Vec<Model>, Option<String>) {
    let models = neo
        .usable_providers()
        .map(|provider| Model {
            key: provider.slot.to_string(),
            label: provider.label.to_string(),
            description: provider.description.to_string(),
            access: provider.access.as_str().into(),
            allowed: provider.access.allows(plan),
            limited: false,
            multiplier: provider.multiplier(),
            vision: provider.sees_images(),
            thinking: false,
            window: None,
        })
        .collect();

    (
        models,
        slot_for(neo, None, plan).ok().map(|slot| slot.to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::neo::{NeoDriver, NeoProvider};

    fn provider(slot: u8, access: NeoAccess) -> NeoProvider {
        NeoProvider {
            slot,
            enabled: true,
            label: format!("Model {slot}").into(),
            description: "".into(),
            driver: NeoDriver::Openai,
            endpoint: "https://api.example.com/v1".into(),
            api_key: Some("key".into()),
            model: "m".into(),
            max_tokens: 4096,
            timeout: 30,
            daily_token_cap: 0,
            extra_headers: Default::default(),
            extra_body: Default::default(),
            access,
            multiplier: 2,
            vision: None,
        }
    }

    #[test]
    fn picks_the_model_or_the_first_open_one() {
        let neo = AppSettingsNeo {
            providers: vec![
                provider(1, NeoAccess::Plusplus),
                provider(2, NeoAccess::Everyone),
                provider(3, NeoAccess::Plus),
            ],
            ..Default::default()
        };

        assert_eq!(slot_for(&neo, None, NeoPlan::Free).ok(), Some(2));
        assert_eq!(slot_for(&neo, Some("3"), NeoPlan::Plus).ok(), Some(3));
        assert!(matches!(
            slot_for(&neo, Some("1"), NeoPlan::Plus),
            Err(TurnError::ModelUnavailable { reason, .. }) if reason == "model_not_allowed"
        ));
        assert!(matches!(
            slot_for(&neo, Some("9"), NeoPlan::Plus),
            Err(TurnError::ModelUnavailable { reason, .. }) if reason == "model_unavailable"
        ));

        let (models, default) = catalog(&neo, NeoPlan::Free);
        assert_eq!(models.len(), 3);
        assert!(!models[0].allowed && models[1].allowed);
        assert_eq!(default.as_deref(), Some("2"));

        let members_only = AppSettingsNeo {
            providers: vec![provider(4, NeoAccess::Plus)],
            ..Default::default()
        };
        assert!(slot_for(&members_only, None, NeoPlan::Free).is_err());
        assert_eq!(
            slot_for(&members_only, None, NeoPlan::Plusplus).ok(),
            Some(4)
        );
    }
}
