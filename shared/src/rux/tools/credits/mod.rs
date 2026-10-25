//! plugged.host: rux's credit grant (`give_credits`, the settings' credit granters only), and
//! where everything rux gives is paid ([`payouts`]: noprofit.space while it is connected, else
//! the panel's own credits).
//!
//! A grant is keyed by the order, the person and the amount, so an order that is retried (the
//! model calling again after a timeout, a worker that died halfway) gives nothing twice. The keys
//! start with `rux:grant:calagopus-`, so they never meet the keys another panel sends to the same
//! noprofit.space.

pub mod payouts;

use super::{
    Tool, ToolError, ToolResult, int,
    moderation::{
        common::{self, case_result, member_property},
        service,
    },
    required, schema, string,
};
use crate::rux::{context::Context, noprofit::Unavailable, say::Strings, text};
use payouts::Payer;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The group's lines: `(key, English, German)`.
pub const STRINGS: Strings = &[];

/// A grant's key on noprofit.space (`rux:grant:` and 8 to 50 of `[A-Za-z0-9_-]`) and in the
/// panel's ledger: the same for the same order, person and amount.
pub fn grant_key(order: &str, guild_id: &str, discord_id: &str, credits: i64) -> String {
    let digest = Sha256::digest(format!("{order}|{guild_id}|{discord_id}|{credits}"));

    format!("rux:grant:calagopus-{}", &hex::encode(digest)[..32])
}

/// What a grant did.
struct Granted {
    account: Value,
    amount: i64,
    balance_after: i64,
    created: bool,
}

pub struct GiveCredits;

impl GiveCredits {
    fn max(context: &Context) -> i64 {
        context.config.settings.credit_max_per_grant.max(1)
    }

    fn only_main_admin(context: &Context) -> ToolError {
        ToolError::invalid(format!(
            "Only {}'s main admin can have {} give credits.",
            context.config.app_name, context.config.settings.name
        ))
    }

    async fn through_noprofit(
        context: &Context,
        client: &crate::rux::noprofit::Client,
        key: &str,
        target: &common::Target,
        credits: i64,
        reason: Option<&str>,
    ) -> Result<Granted, ToolError> {
        let granted_by = context
            .actor_account_id
            .ok_or_else(|| Self::only_main_admin(context))?;
        let answer = client
            .post(
                "/credits",
                &json!({
                    "key": key,
                    "discord_id": target.id,
                    "amount": credits,
                    "reason": reason,
                    "granted_by": granted_by,
                }),
                15,
            )
            .await
            .map_err(|Unavailable(why)| {
                if why.contains("(not_linked)") {
                    ToolError::Invalid(format!(
                        "{} has not linked Discord to a {} account, so they cannot get credits. They can link it at {}.",
                        target.mention(),
                        context.config.portal,
                        context.config.link_url
                    ))
                } else if why.contains("(not_admin)") {
                    ToolError::Invalid(format!(
                        "{} does not know this person as an admin, so no credits were given.",
                        context.config.portal
                    ))
                } else {
                    ToolError::Unavailable(Unavailable(why))
                }
            })?;

        Ok(Granted {
            account: json!({
                "account_id": answer["account_id"],
                "name": text::name(answer["name"].as_str().unwrap_or_default(), 64),
            }),
            amount: answer["amount"].as_i64().unwrap_or(credits),
            balance_after: answer["balance_after"].as_i64().unwrap_or_default(),
            created: answer["created"].as_bool() != Some(false),
        })
    }

    async fn through_panel(
        context: &Context,
        key: &str,
        target: &common::Target,
        credits: i64,
        reason: Option<&str>,
    ) -> Result<Granted, ToolError> {
        let state = &context.state;
        let Some(user_uuid) = payouts::panel_user(state, &target.id).await? else {
            return Err(ToolError::Invalid(format!(
                "{} has not linked Discord to a {} account, so they cannot get credits. They can link it at {}.",
                target.mention(),
                context.config.app_name,
                context.config.link_url
            )));
        };

        let mut transaction = state.database.write().begin().await?;
        let Some(username): Option<String> =
            sqlx::query_scalar("SELECT username FROM users WHERE uuid = $1 FOR UPDATE")
                .bind(user_uuid)
                .fetch_optional(&mut *transaction)
                .await?
        else {
            return Err(ToolError::invalid("Their panel account is gone."));
        };
        let account = json!({ "user": user_uuid, "name": text::name(&username, 64) });

        let earlier: Option<i64> = sqlx::query_scalar(
            "SELECT balance_after FROM user_credit_transactions
            WHERE user_uuid = $1 AND data->>'grant_key' = $2 LIMIT 1",
        )
        .bind(user_uuid)
        .bind(key)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(balance_after) = earlier {
            return Ok(Granted {
                account,
                amount: credits,
                balance_after,
                created: false,
            });
        }

        let description = reason.map_or_else(
            || format!("From {} on Discord", text::name(&context.actor_name, 64)),
            str::to_string,
        );
        let balance_after = crate::economy::change(
            &mut transaction,
            crate::economy::Currency::Credits,
            user_uuid,
            credits,
            crate::economy::TYPE_ADMIN,
            &description,
            json!({
                "grant_key": key,
                "discord_id": target.id,
                "granted_by_discord_id": context.actor_discord_id,
                "via": "rux",
            }),
        )
        .await?
        .ok_or_else(|| ToolError::Failed(anyhow::anyhow!("the credits could not be added")))?;
        transaction.commit().await?;

        Ok(Granted {
            account,
            amount: credits,
            balance_after,
            created: true,
        })
    }
}

#[async_trait::async_trait]
impl Tool for GiveCredits {
    fn name(&self) -> &'static str {
        "give_credits"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Give credits to one person's {} account (they must have linked Discord there). Only {}'s main admin can order this; tell anyone else to ask them. At most {} credits at once.",
            context.config.portal,
            context.config.app_name,
            Self::max(context)
        )
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "member": member_property("Who gets them: <@mention>, user id, or their name."),
                "credits": { "type": "integer", "description": "How many credits (the limit is in this tool's description)." },
                "reason": { "type": "string", "description": "Why, in a few words. Shown in their credit history." },
            }),
            &["member", "credits"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let Some(order) = context
            .source_message_id
            .clone()
            .or_else(|| context.order_id.map(|id| id.to_string()))
            .filter(|_| context.order_id.is_some())
        else {
            return Err(Self::only_main_admin(context));
        };
        if !context.config.grants_credits(&context.actor_discord_id) {
            return Err(Self::only_main_admin(context));
        }

        let credits = int(arguments, "credits").unwrap_or(0);
        let max = Self::max(context);
        if !(1..=max).contains(&credits) {
            return Err(ToolError::invalid(format!(
                "Give between 1 and {max} credits at once."
            )));
        }

        let target = common::member_or_user(context, &required(arguments, "member")?).await?;
        let reason = string(arguments, "reason").map(|reason| text::cut(&reason, 200, ""));
        let key = grant_key(&order, &context.guild_id, &target.id, credits);

        let granted = match payouts::payer(&context.state).await? {
            Payer::Noprofit(client) => {
                Self::through_noprofit(context, &client, &key, &target, credits, reason.as_deref())
                    .await?
            }
            Payer::Panel => {
                Self::through_panel(context, &key, &target, credits, reason.as_deref()).await?
            }
        };

        if !granted.created {
            return Ok(ToolResult::json(&json!({
                "already_given": true,
                "note": "This order already gave them these credits; nothing more was given.",
                "member": target.mention(),
                "credits": granted.amount,
                "account": granted.account,
                "their_balance_now": granted.balance_after,
            })));
        }

        let case = service::credits(
            context,
            &target,
            granted.amount,
            reason.as_deref(),
            granted.account.clone(),
        )
        .await?;

        Ok(ToolResult::json(&case_result(
            &case,
            json!({
                "credits": granted.amount,
                "account": granted.account,
                "their_balance_now": granted.balance_after,
            }),
        )))
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(GiveCredits)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_keys_are_what_noprofit_takes_and_stable_per_order() {
        let key = grant_key("100000000000000007", "1", "100000000000000002", 500);
        let pattern = regex::Regex::new(r"^rux:grant:[A-Za-z0-9_-]{8,50}$").unwrap();
        assert!(pattern.is_match(&key), "{key}");
        assert!(key.starts_with("rux:grant:calagopus-"));

        assert_eq!(
            key,
            grant_key("100000000000000007", "1", "100000000000000002", 500)
        );
        assert_ne!(
            key,
            grant_key("100000000000000007", "1", "100000000000000002", 501)
        );
        assert_ne!(
            key,
            grant_key("100000000000000008", "1", "100000000000000002", 500)
        );
    }
}
