//! plugged.host: rux's case log (`rux_cases`): every moderation action is a case (who ordered it,
//! on whom, why, on which server), posted to that server's mod log channel when one is set. Bans
//! for a while are lifted here when their time is up.

use super::super::ToolError;
use crate::rux::{
    config::Config,
    context::Context,
    discord::Rest,
    models::{CASE_COLUMNS, Case},
    say, text, time,
};
use chrono::NaiveDateTime;
use serde_json::{Map, Value, json};

/// Every action a case can have; each has a `log.<action>` line.
pub const ACTIONS: [&str; 23] = [
    "warn",
    "timeout",
    "untimeout",
    "kick",
    "ban",
    "unban",
    "delete",
    "purge",
    "slowmode",
    "lock",
    "unlock",
    "role_add",
    "role_remove",
    "rule_add",
    "rule_remove",
    "rule_edit",
    "watch_add",
    "watch_remove",
    "mod_role_add",
    "mod_role_remove",
    "clear_warnings",
    "nickname",
    "credits",
];

/// A case to write.
#[derive(Debug, Clone, Default)]
pub struct NewCase {
    pub action: String,
    pub target_id: Option<String>,
    pub target_name: Option<String>,
    pub channel_id: Option<String>,
    pub reason: Option<String>,
    pub seconds: Option<i64>,
    pub automatic: bool,
    pub rule_id: Option<i64>,
    /// A ban for a while: when rux lifts it (UTC).
    pub expires: Option<NaiveDateTime>,
    pub meta: Map<String, Value>,
}

impl NewCase {
    pub fn new(action: &str) -> Self {
        Self {
            action: action.to_string(),
            ..Default::default()
        }
    }

    pub fn on(mut self, target: Option<&super::common::Target>) -> Self {
        if let Some(target) = target {
            self.target_id = Some(target.id.clone());
            self.target_name = Some(target.name.clone());
        }
        self
    }

    pub fn reason(mut self, reason: Option<&str>) -> Self {
        self.reason = reason
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
            .map(str::to_string);
        self
    }

    pub fn channel(mut self, channel_id: &str) -> Self {
        self.channel_id = Some(channel_id.to_string());
        self
    }

    pub fn seconds(mut self, seconds: Option<i64>) -> Self {
        self.seconds = seconds;
        self
    }

    pub fn automatic(mut self, automatic: bool, rule_id: Option<i64>) -> Self {
        self.automatic = automatic || rule_id.is_some();
        self.rule_id = rule_id;
        self
    }

    pub fn meta(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.meta.insert(key.to_string(), value.into());
        self
    }
}

impl Case {
    /// Writes a case as `moderator` did it.
    pub async fn insert(
        database: &crate::database::Database,
        guild_id: &str,
        moderator: (&str, Option<i64>, Option<uuid::Uuid>),
        case: &NewCase,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "INSERT INTO rux_cases
                (guild_id, action, target_id, target_name, channel_id, moderator_discord_id, moderator_account_id,
                 moderator_user_uuid, reason, duration_seconds, automatic, rule_id, expires, meta)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING {CASE_COLUMNS}"
        )))
        .bind(guild_id)
        .bind(&case.action)
        .bind(&case.target_id)
        .bind(case.target_name.as_deref().map(|name| text::cut(name, 100, "")))
        .bind(&case.channel_id)
        .bind(moderator.0)
        .bind(moderator.1)
        .bind(moderator.2)
        .bind(case.reason.as_deref().map(|reason| text::cut(reason, 500, "")))
        .bind(case.seconds.map(|seconds| seconds.clamp(0, i32::MAX as i64) as i32))
        .bind(case.automatic)
        .bind(case.rule_id)
        .bind(case.expires)
        .bind((!case.meta.is_empty()).then(|| Value::Object(case.meta.clone())))
        .fetch_one(database.write())
        .await
    }

    /// Someone's cases on one server, newest first.
    pub async fn history(
        database: &crate::database::Database,
        guild_id: &str,
        discord_id: &str,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {CASE_COLUMNS} FROM rux_cases WHERE guild_id = $1 AND target_id = $2
            ORDER BY id DESC LIMIT $3"
        )))
        .bind(guild_id)
        .bind(discord_id)
        .bind(limit)
        .fetch_all(database.read())
        .await
    }

    /// The warnings that count against someone on one server.
    pub async fn warning_count(
        database: &crate::database::Database,
        guild_id: &str,
        discord_id: &str,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM rux_cases
            WHERE guild_id = $1 AND target_id = $2 AND action = 'warn' AND revoked IS NULL",
        )
        .bind(guild_id)
        .bind(discord_id)
        .fetch_one(database.read())
        .await
    }

    /// Whether the last lock or unlock of a channel was a lock rux put on.
    pub async fn locked_by_rux(
        database: &crate::database::Database,
        guild_id: &str,
        channel_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let last: Option<String> = sqlx::query_scalar(
            "SELECT action FROM rux_cases
            WHERE guild_id = $1 AND channel_id = $2 AND action IN ('lock', 'unlock')
            ORDER BY id DESC LIMIT 1",
        )
        .bind(guild_id)
        .bind(channel_id)
        .fetch_optional(database.read())
        .await?;

        Ok(last.as_deref() == Some("lock"))
    }

    fn meta_value(&self, key: &str) -> Option<&Value> {
        self.meta
            .as_ref()
            .and_then(|meta| meta.get(key))
            .filter(|value| !value.is_null())
    }

    /// The case in one line for the model: number, action, when, who, why, and what the member
    /// said when rux caught them.
    pub fn line(&self, with_member: bool, zone: chrono_tz::Tz) -> String {
        let mut line = format!("#{} {}", self.id, self.action);
        if self.automatic {
            line.push_str(&match self.rule_id {
                Some(rule) => format!(" (automatic, rule #{rule})"),
                None => " (automatic)".to_string(),
            });
        }
        if self.revoked.is_some() {
            line.push_str(" (cleared)");
        }
        line.push(' ');
        line.push_str(&time::human(self.created.and_utc(), zone));
        if with_member && let Some(target) = &self.target_id {
            line.push_str(&format!(" on <@{target}>"));
        }
        if with_member && !self.automatic {
            line.push_str(&format!(" by <@{}>", self.moderator_discord_id));
        }
        if let Some(channel) = &self.channel_id {
            line.push_str(&format!(" in <#{channel}>"));
        }
        if let Some(seconds) = self.duration_seconds.filter(|seconds| *seconds > 0) {
            line.push_str(&format!(" for {}", time::human_duration(seconds as i64)));
        }
        if let Some(deleted) = self.meta_value("deleted") {
            line.push_str(&format!(", {deleted} deleted"));
        }
        if let Some(reason) = self.reason.as_deref().filter(|reason| !reason.is_empty()) {
            line.push_str(&format!(": {reason}"));
        }
        if let Some(said) = self
            .meta_value("said")
            .and_then(Value::as_str)
            .filter(|said| !said.is_empty())
        {
            line.push_str(&format!(" (they said: \"{}\")", text::untrusted(said, 200)));
        }

        line
    }
}

/// Writes a case for this order (or for rux acting by itself: then what it caught is kept with
/// it) and posts it to the server's mod log.
pub async fn record(context: &Context, mut case: NewCase) -> Result<Case, ToolError> {
    if let Some(evidence) = context
        .evidence
        .as_deref()
        .map(str::trim)
        .filter(|evidence| !evidence.is_empty())
    {
        case.meta
            .entry("said")
            .or_insert_with(|| json!(text::cut(evidence, 300, "")));
        case.meta
            .entry("message")
            .or_insert_with(|| json!(context.source_message_id));
        if case.channel_id.is_none() {
            case.channel_id = Some(context.channel_id.clone());
        }
    }

    let row = Case::insert(
        &context.state.database,
        &context.guild_id,
        (
            &context.actor_discord_id,
            context.actor_account_id,
            context.actor_user_uuid,
        ),
        &case,
    )
    .await?;
    tracing::info!(
        case = row.id,
        action = %row.action,
        member = row.target_id.as_deref().unwrap_or_default(),
        by = %context.actor_discord_id,
        "rux: moderation"
    );
    post_to_mod_log(
        &context.state.database,
        &context.config,
        &context.rest,
        &row,
    )
    .await;

    Ok(row)
}

/// `1234567` as people read it: `1,234,567` (English) or `1.234.567` (German).
pub fn thousands(number: i64, locale: &str) -> String {
    let separator = if locale == "de" { '.' } else { ',' };
    let digits = number.unsigned_abs().to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(separator);
        }
        out.push(digit);
    }

    if number < 0 { format!("-{out}") } else { out }
}

/// The mod log's "Details" of a case.
fn details(config: &Config, case: &Case, warnings: Option<i64>) -> Vec<String> {
    let mut details = Vec::new();
    let number = |key: &str| case.meta_value(key).and_then(Value::as_i64);
    let string = |key: &str| {
        case.meta_value(key)
            .and_then(Value::as_str)
            .map(str::to_string)
    };

    if let Some(deleted) = number("deleted") {
        details.push(config.say("log.detail_deleted", &[("count", &deleted.to_string())]));
    }
    if let Some(role) = string("role_name") {
        details.push(role);
    }
    if let (Some(cleared), Some(left)) = (number("cleared"), number("left")) {
        details.push(config.say(
            "log.detail_cleared",
            &[
                ("cleared", &cleared.to_string()),
                ("left", &left.to_string()),
            ],
        ));
    }
    if case.action == "nickname" {
        let none = config.say("log.no_nickname", &[]);
        details.push(format!(
            "{} → {}",
            string("from").unwrap_or_else(|| none.clone()),
            string("to").unwrap_or(none)
        ));
    }
    if let Some(changed) = case.meta_value("changed").and_then(Value::as_array) {
        let fields: Vec<&str> = changed.iter().filter_map(Value::as_str).collect();
        details.push(config.say("log.detail_changed", &[("fields", &fields.join(", "))]));
    }
    if let Some(credits) = number("credits") {
        details.push(config.say(
            "log.detail_credits",
            &[("credits", &thousands(credits, config.locale()))],
        ));
    }
    if let Some(warnings) = warnings {
        details.push(say::say_count(
            config.locale(),
            "log.warnings",
            warnings,
            &[],
        ));
    }

    details
}

/// The mod log message of a case.
pub fn mod_log_payload(config: &Config, case: &Case, warnings: Option<i64>) -> Value {
    let mut fields = Vec::new();
    let field = |name: &str, value: String, inline: bool| json!({ "name": config.say(name, &[]), "value": text::cut(&value, 1024, ""), "inline": inline });

    if let Some(target) = &case.target_id {
        fields.push(field("log.member", format!("<@{target}> ({target})"), true));
    }
    if let Some(channel) = &case.channel_id {
        fields.push(field("log.channel", format!("<#{channel}>"), true));
    }
    fields.push(field(
        "log.by",
        if case.automatic {
            match case.rule_id {
                Some(rule) => config.say("log.by_rule", &[("rule", &rule.to_string())]),
                None => config.say("log.by_rux", &[]),
            }
        } else {
            format!("<@{}>", case.moderator_discord_id)
        },
        true,
    ));
    if let Some(seconds) = case.duration_seconds
        && ["timeout", "slowmode", "ban"].contains(&case.action.as_str())
    {
        fields.push(field(
            "log.duration",
            if seconds == 0 {
                config.say("log.off", &[])
            } else {
                time::human_duration(seconds as i64)
            },
            true,
        ));
    }
    let details = details(config, case, warnings);
    if !details.is_empty() {
        fields.push(field("log.details", details.join(", "), true));
    }
    if let Some(reason) = &case.reason {
        fields.push(field("log.reason", reason.clone(), false));
    }
    if let Some(said) = case
        .meta_value("said")
        .and_then(Value::as_str)
        .filter(|said| !said.is_empty())
    {
        fields.push(field("log.said", said.to_string(), false));
    }

    let mut title = config.say(&format!("log.{}", case.action), &[]);
    if let Some(name) = &case.target_name {
        title.push_str(": ");
        title.push_str(name);
    }

    json!({
        "embeds": [{
            "title": text::cut(&title, 256, ""),
            "fields": fields,
            "footer": { "text": config.say("log.case", &[("id", &case.id.to_string())]) },
            "timestamp": case.created.and_utc().to_rfc3339(),
        }],
        "allowed_mentions": { "parse": [] },
    })
}

/// Posts a case to the mod log of its server; none for a server rux no longer works on. A failure
/// is only logged: the action itself is done.
pub async fn post_to_mod_log(
    database: &crate::database::Database,
    config: &Config,
    rest: &Rest,
    case: &Case,
) {
    let Some(channel) = config
        .guild(&case.guild_id)
        .and_then(|guild| guild.mod_log_channel_id.clone())
    else {
        return;
    };

    let warnings = match (&case.target_id, case.action.as_str()) {
        (Some(target), "warn") => Case::warning_count(database, &case.guild_id, target)
            .await
            .ok(),
        _ => None,
    };

    if let Err(err) = rest
        .create_message(&channel, &mod_log_payload(config, case, warnings))
        .await
    {
        tracing::warn!(case = case.id, "rux: could not post to the mod log: {err}");
    }
}

/// Lifts the bans for a while whose time is up. One that was lifted by hand on Discord is just
/// marked done.
pub async fn lift_expired_bans(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
) -> Result<usize, anyhow::Error> {
    let bans = sqlx::query_as::<_, Case>(sqlx::AssertSqlSafe(format!(
        "SELECT {CASE_COLUMNS} FROM rux_cases
        WHERE action = 'ban' AND expires IS NOT NULL AND resolved IS NULL AND expires <= $1
        ORDER BY id LIMIT 50"
    )))
    .bind(chrono::Utc::now().naive_utc())
    .fetch_all(state.database.read())
    .await?;

    let mut lifted = 0;
    for ban in bans {
        let id = ban.id.to_string();
        if let Some(target) = &ban.target_id {
            let reason = config.say("log.ban_ran_out", &[("id", &id)]);
            if let Err(err) = rest.unban(&ban.guild_id, target, Some(&reason)).await
                && err.code != Some(10026)
            {
                tracing::warn!(
                    case = ban.id,
                    "rux: could not lift a ban that ran out: {err}"
                );
                continue;
            }
        }

        let claimed = sqlx::query(
            "UPDATE rux_cases SET resolved = NOW(), updated = NOW() WHERE id = $1 AND resolved IS NULL",
        )
        .bind(ban.id)
        .execute(state.database.write())
        .await?
        .rows_affected()
            > 0;
        if !claimed || ban.target_id.is_none() {
            continue;
        }
        lifted += 1;

        let case = Case::insert(
            &state.database,
            &ban.guild_id,
            (
                &ban.moderator_discord_id,
                ban.moderator_account_id,
                ban.moderator_user_uuid,
            ),
            &NewCase {
                action: "unban".into(),
                target_id: ban.target_id.clone(),
                target_name: ban.target_name.clone(),
                reason: Some(config.say("log.ban_ended", &[("id", &id)])),
                automatic: true,
                ..Default::default()
            },
        )
        .await?;
        post_to_mod_log(&state.database, config, rest, &case).await;
    }

    Ok(lifted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::rux::AppSettingsRux;

    fn config(locale: &str) -> Config {
        let settings = AppSettingsRux {
            locale: locale.into(),
            guild_id: "100000000000000001".into(),
            ..Default::default()
        };

        Config {
            guilds: crate::rux::config::guilds(&settings, "", "Example"),
            settings,
            enabled: true,
            app_name: "Example".into(),
            portal: "Example".into(),
            link_url: "https://panel.example.com".into(),
            boost_two_role_id: String::new(),
            managed: None,
            managed_bot_token: None,
            memberships: Default::default(),
        }
    }

    fn case(action: &str, meta: Value) -> Case {
        let created = chrono::DateTime::from_timestamp(1_791_108_000, 0)
            .unwrap()
            .naive_utc();

        Case {
            id: 12,
            guild_id: "100000000000000001".into(),
            action: action.into(),
            target_id: Some("100000000000000002".into()),
            target_name: Some("Ann".into()),
            channel_id: None,
            moderator_discord_id: "100000000000000003".into(),
            moderator_account_id: None,
            moderator_user_uuid: None,
            reason: Some("spam".into()),
            duration_seconds: Some(7200),
            automatic: false,
            rule_id: None,
            expires: None,
            resolved: None,
            revoked: None,
            meta: Some(meta),
            created,
            updated: created,
        }
    }

    #[test]
    fn every_action_has_a_log_line() {
        for action in ACTIONS {
            for locale in ["en", "de"] {
                let key = format!("log.{action}");
                assert_ne!(say::say(locale, &key, &[]), key, "{key}");
            }
        }
    }

    #[test]
    fn lines_say_what_happened() {
        let mut case = case(
            "timeout",
            json!({"said": "buy <@100000000000000009> now", "deleted": 3}),
        );
        assert_eq!(
            case.line(true, chrono_tz::UTC),
            "#12 timeout Sun 4 Oct 2026, 10:00 UTC time on <@100000000000000002> by <@100000000000000003> for 2 hours, 3 deleted: spam (they said: \"buy @100000000000000009 now\")"
        );

        case.automatic = true;
        case.rule_id = Some(4);
        case.revoked = Some(case.created);
        case.meta = None;
        case.duration_seconds = None;
        assert_eq!(
            case.line(false, chrono_tz::UTC),
            "#12 timeout (automatic, rule #4) (cleared) Sun 4 Oct 2026, 10:00 UTC time: spam"
        );
    }

    #[test]
    fn the_mod_log_shows_the_details() {
        let english = config("en");
        let payload = mod_log_payload(&english, &case("warn", json!({"said": "hi"})), Some(1));
        let embed = &payload["embeds"][0];
        assert_eq!(embed["title"], "Warned: Ann");
        assert_eq!(embed["footer"]["text"], "Case #12");
        let fields: Vec<String> = embed["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| {
                format!(
                    "{}={}",
                    field["name"].as_str().unwrap(),
                    field["value"].as_str().unwrap()
                )
            })
            .collect();
        assert_eq!(
            fields,
            vec![
                "Member=<@100000000000000002> (100000000000000002)",
                "Ordered by=<@100000000000000003>",
                "Details=1 warning in total",
                "Reason=spam",
                "Message=hi",
            ]
        );
        assert_eq!(payload["allowed_mentions"]["parse"], json!([]));

        let mut slowmode = case("slowmode", json!({}));
        slowmode.duration_seconds = Some(0);
        slowmode.automatic = true;
        slowmode.target_id = None;
        slowmode.target_name = None;
        let payload = mod_log_payload(&config("de"), &slowmode, None);
        let embed = &payload["embeds"][0];
        assert_eq!(embed["title"], "Slowmode geändert");
        assert_eq!(embed["fields"][0]["value"], "rux, automatisch");
        assert_eq!(embed["fields"][1]["value"], "aus");

        let credits = case("credits", json!({"credits": 12500}));
        let payload = mod_log_payload(&config("de"), &credits, None);
        assert_eq!(
            payload["embeds"][0]["fields"][2]["value"],
            "+12.500 Credits"
        );
    }

    #[test]
    fn numbers_are_grouped() {
        assert_eq!(thousands(0, "en"), "0");
        assert_eq!(thousands(999, "en"), "999");
        assert_eq!(thousands(1000, "en"), "1,000");
        assert_eq!(thousands(1_234_567, "de"), "1.234.567");
        assert_eq!(thousands(-12_000, "en"), "-12,000");
    }
}
