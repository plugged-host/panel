//! plugged.host: the giveaway tools (`create_giveaway`, `list_giveaways`, `edit_giveaway`,
//! `end_giveaway`, `cancel_giveaway`, `reroll_giveaway`). Each acts on the order's server's
//! giveaways only; listing is open to moderators, everything else is for admins.

use super::{
    super::{Tool, ToolError, ToolResult, int, required, schema, string},
    service,
};
use crate::rux::{
    access::Level,
    context::Context,
    models::{GIVEAWAY_COLUMNS, Giveaway, giveaway_status as status},
};
use serde_json::{Map, Value, json};

/// The giveaway the model named, or the only one in `statuses` when it named none; only the
/// order's server's giveaways.
async fn giveaway(
    context: &Context,
    arguments: &Value,
    statuses: &[&str],
) -> Result<Giveaway, ToolError> {
    if let Some(id) = int(arguments, "giveaway_id") {
        return sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
            "SELECT {GIVEAWAY_COLUMNS} FROM rux_giveaways WHERE id = $1 AND guild_id = $2"
        )))
        .bind(id)
        .bind(&context.guild_id)
        .fetch_optional(context.state.database.read())
        .await?
        .ok_or_else(|| {
            ToolError::Invalid(format!(
                "There is no giveaway #{id} on this server. Use list_giveaways."
            ))
        });
    }

    let statuses: Vec<String> = statuses.iter().map(|status| status.to_string()).collect();
    let mut candidates = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "SELECT {GIVEAWAY_COLUMNS} FROM rux_giveaways
        WHERE guild_id = $1 AND status = ANY($2)
        ORDER BY id DESC LIMIT 6"
    )))
    .bind(&context.guild_id)
    .bind(&statuses)
    .fetch_all(context.state.database.read())
    .await?;

    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(ToolError::Invalid(format!(
            "There is no {} giveaway.",
            statuses.join(" or ")
        ))),
        _ => Err(ToolError::Invalid(format!(
            "Several giveaways could be meant: {}. Ask which one, or pass giveaway_id.",
            candidates
                .iter()
                .map(|giveaway| format!(
                    "#{} {} ({})",
                    giveaway.id, giveaway.prize, giveaway.status
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn id_property() -> Value {
    json!({ "type": "integer", "description": "Giveaway number. Leave out when there is only one that fits." })
}

/// `{flag: true}` followed by the giveaway's summary.
fn with_flag(flag: &str, summary: Value) -> Value {
    let mut out = Map::new();
    out.insert(flag.to_string(), Value::Bool(true));
    if let Value::Object(summary) = summary {
        out.extend(summary);
    }

    Value::Object(out)
}

pub struct CreateGiveaway;

#[async_trait::async_trait]
impl Tool for CreateGiveaway {
    fn name(&self) -> &'static str {
        "create_giveaway"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Start a giveaway on this server: posts it with an Enter button. The prize is credits (default), {app} tokens, or days of {plus} or {plusplus} (prize_kind), nothing else. Entering always requires a Discord account linked to {} (automatic). At the end {} draws the winners and pays them automatically.",
            context.config.portal,
            context.config.settings.name,
            app = context.config.app_name,
            plus = context.config.memberships.plus,
            plusplus = context.config.memberships.plusplus,
        )
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "prize_kind": { "type": "string", "enum": ["credits", "tokens", "plus_days", "plusplus_days"], "description": "What each winner gets: credits (default), tokens, or days of the first (plus_days) or second (plusplus_days) membership." },
                "credits": { "type": "integer", "description": "The amount each winner gets, in the prize_kind's unit: credits, tokens or days." },
                "ends_at": { "type": "string", "description": "When it ends: \"YYYY-MM-DD HH:MM\" in rux's time zone, or a duration from now like \"2h\" or \"3d\"." },
                "winners": { "type": "integer", "description": "How many winners. Default 1." },
                "channel": { "type": "string", "description": "Channel to post in (#name, <#id> or id). Leave out to use the default giveaway channel." },
                "exclude_recent_winners": { "type": "integer", "description": "Keep out everyone who won one of the last N giveaways. 0 = nobody kept out." },
                "title": { "type": "string", "description": "Optional headline naming the occasion, e.g. \"Halloween giveaway\"; never a prize other than its own (no RAM, Nitro or roles; tokens or the membership only when that is the prize). Default: the prize, like \"500 credits\" or \"30 days of Plus\"." },
            }),
            &["credits", "ends_at"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let credits = required(arguments, "credits")?;
        let credits = int(arguments, "credits").ok_or_else(|| {
            ToolError::Invalid(format!(
                "credits must be a whole number, not \"{credits}\"."
            ))
        })?;

        let giveaway = service::create(
            context,
            service::NewGiveaway {
                kind: string(arguments, "prize_kind").unwrap_or_else(|| "credits".into()),
                credits,
                ends_at: required(arguments, "ends_at")?,
                winners: int(arguments, "winners").unwrap_or(1),
                channel: string(arguments, "channel"),
                exclude_recent: int(arguments, "exclude_recent_winners").unwrap_or(0),
                title: string(arguments, "title"),
            },
        )
        .await?;

        Ok(ToolResult::json(&with_flag(
            "created",
            service::summary(&context.state, &context.config, &giveaway).await?,
        )))
    }
}

pub struct ListGiveaways;

#[async_trait::async_trait]
impl Tool for ListGiveaways {
    fn name(&self) -> &'static str {
        "list_giveaways"
    }

    fn description(&self, _context: &Context) -> String {
        "This server's running giveaways and its most recent ended or cancelled ones, with entries, winners and payout.".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "giveaway_id": { "type": "integer", "description": "Show only this giveaway." },
            }),
            &[],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let (state, config) = (&context.state, &context.config);
        if int(arguments, "giveaway_id").is_some() {
            let giveaway = giveaway(context, arguments, &[]).await?;

            return Ok(ToolResult::json(
                &service::summary(state, config, &giveaway).await?,
            ));
        }

        let running = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
            "SELECT {GIVEAWAY_COLUMNS} FROM rux_giveaways
            WHERE guild_id = $1 AND status IN ('active', 'drawing')
            ORDER BY ends LIMIT 10"
        )))
        .bind(&context.guild_id)
        .fetch_all(state.database.read())
        .await?;
        let recent = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
            "SELECT {GIVEAWAY_COLUMNS} FROM rux_giveaways
            WHERE guild_id = $1 AND status IN ('ended', 'cancelled')
            ORDER BY id DESC LIMIT 5"
        )))
        .bind(&context.guild_id)
        .fetch_all(state.database.read())
        .await?;

        let mut out = json!({ "running": [], "recent": [] });
        for (key, giveaways) in [("running", running), ("recent", recent)] {
            let mut summaries = Vec::new();
            for giveaway in &giveaways {
                summaries.push(service::summary(state, config, giveaway).await?);
            }
            out[key] = Value::Array(summaries);
        }

        Ok(ToolResult::json(&out))
    }
}

pub struct EditGiveaway;

#[async_trait::async_trait]
impl Tool for EditGiveaway {
    fn name(&self) -> &'static str {
        "edit_giveaway"
    }

    fn description(&self, _context: &Context) -> String {
        "Change a running giveaway: end time, the amount per winner (in its prize's unit: credits, tokens or days), number of winners or headline. The prize kind stays. Its message updates.".into()
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "giveaway_id": id_property(),
                "ends_at": { "type": "string", "description": "New end: \"YYYY-MM-DD HH:MM\" in rux's time zone, or a duration from now." },
                "credits": { "type": "integer", "description": "New amount per winner, in the giveaway's prize unit (credits, tokens or days)." },
                "winners": { "type": "integer", "description": "New number of winners." },
                "title": { "type": "string", "description": "New headline." },
            }),
            &[],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let found = giveaway(context, arguments, &[status::ACTIVE]).await?;
        let updated = service::update(
            context,
            &found,
            service::Changes {
                credits: int(arguments, "credits"),
                ends_at: string(arguments, "ends_at"),
                winners: int(arguments, "winners"),
                title: string(arguments, "title"),
            },
        )
        .await?;

        Ok(ToolResult::json(&with_flag(
            "updated",
            service::summary(&context.state, &context.config, &updated).await?,
        )))
    }
}

pub struct EndGiveaway;

#[async_trait::async_trait]
impl Tool for EndGiveaway {
    fn name(&self) -> &'static str {
        "end_giveaway"
    }

    fn description(&self, _context: &Context) -> String {
        "End a running giveaway now: draws and pays the winners right away.".into()
    }

    fn parameters(&self) -> Value {
        schema(json!({ "giveaway_id": id_property() }), &[])
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let found = giveaway(context, arguments, &[status::ACTIVE]).await?;
        let Some(ended) = service::end_now(context, &found).await? else {
            return Ok(ToolResult::error(format!(
                "Giveaway #{} could not be drawn yet (it could not be told who is linked to {}); it will be drawn automatically within a minute or two.",
                found.id, context.config.portal
            )));
        };

        Ok(ToolResult::json(&with_flag(
            "ended",
            service::summary(&context.state, &context.config, &ended).await?,
        )))
    }
}

pub struct CancelGiveaway;

#[async_trait::async_trait]
impl Tool for CancelGiveaway {
    fn name(&self) -> &'static str {
        "cancel_giveaway"
    }

    fn description(&self, _context: &Context) -> String {
        "Cancel a running giveaway: nobody wins, nothing is paid.".into()
    }

    fn parameters(&self) -> Value {
        schema(json!({ "giveaway_id": id_property() }), &[])
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let found = giveaway(context, arguments, &[status::ACTIVE]).await?;
        let cancelled = service::cancel(context, &found).await?;

        Ok(ToolResult::json(&with_flag(
            "cancelled",
            service::summary(&context.state, &context.config, &cancelled).await?,
        )))
    }
}

pub struct RerollGiveaway;

#[async_trait::async_trait]
impl Tool for RerollGiveaway {
    fn name(&self) -> &'static str {
        "reroll_giveaway"
    }

    fn description(&self, _context: &Context) -> String {
        "Draw extra winners for an ended giveaway among the entries that have not won. They are paid too; earlier winners keep their prize.".into()
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "giveaway_id": id_property(),
                "count": { "type": "integer", "description": "How many new winners. Default 1." },
            }),
            &[],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let found = giveaway(context, arguments, &[status::ENDED]).await?;
        let winners =
            service::reroll(context, &found, int(arguments, "count").unwrap_or(1)).await?;
        let found = Giveaway::by_id(&context.state.database, found.id)
            .await?
            .unwrap_or(found);

        let mut out = Map::new();
        out.insert(
            "new_winners".into(),
            json!(
                winners
                    .iter()
                    .map(|id| format!("<@{id}>"))
                    .collect::<Vec<_>>()
            ),
        );
        if let Value::Object(summary) =
            service::summary(&context.state, &context.config, &found).await?
        {
            out.extend(summary);
        }

        Ok(ToolResult::json(&Value::Object(out)))
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(CreateGiveaway),
        Box::new(ListGiveaways),
        Box::new(EditGiveaway),
        Box::new(EndGiveaway),
        Box::new(CancelGiveaway),
        Box::new(RerollGiveaway),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_comes_first() {
        let out = with_flag("created", json!({ "id": 3, "status": "active" }));
        assert_eq!(
            serde_json::to_string(&out).unwrap(),
            r#"{"created":true,"id":3,"status":"active"}"#
        );
    }

    #[test]
    fn only_listing_is_for_moderators() {
        for tool in tools() {
            let expected = if tool.name() == "list_giveaways" {
                Level::Moderator
            } else {
                Level::Admin
            };
            assert_eq!(tool.level(), expected, "{}", tool.name());
        }
    }
}
