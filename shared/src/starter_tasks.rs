//! plugged.host: the first-week tasks on the Tokens page, "Getting started"
//! (docs/features/economy.md#getting-started).
//!
//! An account younger than **Days Shown** (14 by default) sees a short list: link Discord (while
//! the Discord section is on), create a server, start it, earn tokens once. Each pays **Tokens
//! per Task** once per account, checked when it is claimed. The ledger row (`starter:<task>`) is
//! the record of the claim, written with the account row locked, so a task never pays twice. The
//! list is gone once everything is claimed or the days passed.

use crate::{
    economy::{Currency, EconomyError, change},
    models::user::User,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The ledger type of a task's tokens is this followed by the task's key.
pub const TYPE_PREFIX: &str = "starter:";

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StarterTask {
    /// Link a Discord account (only while the Discord section is on).
    Discord,
    CreateServer,
    /// Start one of the account's servers once.
    StartServer,
    /// Earn tokens once with a sponsored link or a survey.
    Earn,
}

impl StarterTask {
    /// In the order the page lists them.
    pub const ALL: [Self; 4] = [
        Self::Discord,
        Self::CreateServer,
        Self::StartServer,
        Self::Earn,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Discord => "discord",
            Self::CreateServer => "create_server",
            Self::StartServer => "start_server",
            Self::Earn => "earn",
        }
    }

    /// The ledger row's type.
    pub fn ledger_type(self) -> String {
        format!("{TYPE_PREFIX}{}", self.key())
    }

    /// The ledger row's description.
    fn label(self) -> &'static str {
        match self {
            Self::Discord => "Link Discord",
            Self::CreateServer => "Create a server",
            Self::StartServer => "Start a server",
            Self::Earn => "Earn tokens",
        }
    }
}

/// Whether the tasks are still shown to an account made at `created`: `days` after it (0: never).
pub fn window_open(created: chrono::NaiveDateTime, days: i64, now: chrono::NaiveDateTime) -> bool {
    days > 0 && now < created + chrono::Duration::days(days)
}

#[derive(ToSchema, Serialize)]
pub struct ApiStarterTask {
    pub task: StarterTask,
    /// Done, so it can be claimed (or claimed already).
    pub done: bool,
    pub claimed: bool,
}

/// The "Getting started" list.
#[derive(ToSchema, Serialize)]
pub struct ApiStarter {
    /// Tokens each task pays.
    pub reward: i64,
    /// When the list goes away.
    pub ends: chrono::DateTime<chrono::Utc>,
    pub tasks: Vec<ApiStarterTask>,
}

fn offered(economy: &crate::settings::economy::AppSettingsEconomy) -> bool {
    economy.tokens_enabled && economy.starter_enabled && economy.starter_days > 0
}

/// The tasks offered: linking Discord only while the Discord section is on.
async fn tasks(state: &crate::State) -> Result<Vec<StarterTask>, anyhow::Error> {
    let discord = state.settings.get().await?.discord.active();

    Ok(StarterTask::ALL
        .into_iter()
        .filter(|task| *task != StarterTask::Discord || discord)
        .collect())
}

/// Whether the account did the task.
async fn done(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    task: StarterTask,
) -> Result<bool, anyhow::Error> {
    Ok(match task {
        StarterTask::Discord => {
            let context = crate::discord::Context::load(state).await?;

            context.settings.active()
                && crate::discord::linked_discord_id(state, &context, user_uuid)
                    .await?
                    .is_some()
        }
        StarterTask::CreateServer => {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM servers WHERE owner_uuid = $1)",
            )
                .bind(user_uuid)
                .fetch_one(state.database.read())
                .await?
        }
        // a start by its button, or one let through by the start queue or the waiting room
        StarterTask::StartServer => {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (
                    SELECT 1 FROM server_activities
                    JOIN servers ON servers.uuid = server_activities.server_uuid
                    WHERE servers.owner_uuid = $1 AND (
                        (server_activities.event = 'server:power.action'
                            AND server_activities.data->>'action' IN ('start', 'restart'))
                        OR server_activities.event IN ('server:queue.release', 'server:waiting-room.release')
                    )
                )",
            )
            .bind(user_uuid)
            .fetch_one(state.database.read())
            .await?
        }
        StarterTask::Earn => {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM user_token_claims WHERE user_uuid = $1 AND status = $2)",
            )
            .bind(user_uuid)
            .bind(crate::earn::STATUS_GRANTED)
            .fetch_one(state.database.read())
            .await?
        }
    })
}

/// The tasks the account was paid for, by ledger type.
async fn claimed(
    executor: impl sqlx::PgExecutor<'_>,
    user_uuid: uuid::Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT type FROM user_token_transactions
        WHERE user_uuid = $1 AND type LIKE 'starter:%'",
    )
    .bind(user_uuid)
    .fetch_all(executor)
    .await
}

/// The list; `None` while it is off, once the days passed, or once every task is claimed.
pub async fn status(
    state: &crate::State,
    economy: &crate::settings::economy::AppSettingsEconomy,
    user: &User,
) -> Result<Option<ApiStarter>, anyhow::Error> {
    let days = economy.starter_days.max(0) as i64;
    if !offered(economy) || !window_open(user.created, days, chrono::Utc::now().naive_utc()) {
        return Ok(None);
    }

    let paid = claimed(state.database.write(), user.uuid).await?;
    let mut list = Vec::new();
    for task in tasks(state).await? {
        let is_claimed = paid.iter().any(|kind| *kind == task.ledger_type());

        list.push(ApiStarterTask {
            task,
            done: is_claimed || done(state, user.uuid, task).await?,
            claimed: is_claimed,
        });
    }
    if list.iter().all(|task| task.claimed) {
        return Ok(None);
    }

    Ok(Some(ApiStarter {
        reward: economy.starter_reward.max(1) as i64,
        ends: (user.created + chrono::Duration::days(days)).and_utc(),
        tasks: list,
    }))
}

/// Claims a task: checked now, paid once per account. Answers the tokens paid.
pub async fn claim(
    state: &crate::State,
    user: &User,
    task: StarterTask,
) -> Result<i64, EconomyError> {
    let economy = state.settings.get().await?.economy.clone();
    if !offered(&economy) {
        return Err(EconomyError::Disabled);
    }
    if !window_open(
        user.created,
        economy.starter_days.max(0) as i64,
        chrono::Utc::now().naive_utc(),
    ) {
        return Err(EconomyError::Conflict(
            "the getting started tasks have ended".into(),
        ));
    }
    if !tasks(state).await?.contains(&task) {
        return Err(EconomyError::BadRequest("this task is not offered"));
    }
    if !done(state, user.uuid, task).await? {
        return Err(EconomyError::Conflict(
            "do the task first, then claim it".into(),
        ));
    }

    let reward = economy.starter_reward.max(1) as i64;
    let mut transaction = state.database.write().begin().await?;
    sqlx::query("SELECT 1 FROM users WHERE uuid = $1 FOR UPDATE")
        .bind(user.uuid)
        .execute(&mut *transaction)
        .await?;

    if claimed(&mut *transaction, user.uuid)
        .await?
        .contains(&task.ledger_type())
    {
        return Err(EconomyError::Conflict("this task is paid already".into()));
    }

    change(
        &mut transaction,
        Currency::Tokens,
        user.uuid,
        reward,
        &task.ledger_type(),
        task.label(),
        serde_json::json!({ "task": task.key() }),
    )
    .await?;
    transaction.commit().await?;

    Ok(reward)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32, hour: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    }

    #[test]
    fn the_list_shows_for_the_first_days_only() {
        assert!(window_open(at(1, 12), 14, at(1, 12)));
        assert!(window_open(at(1, 12), 14, at(15, 11)));
        assert!(!window_open(at(1, 12), 14, at(15, 12)));
        assert!(!window_open(at(1, 12), 0, at(1, 12)));
    }

    #[test]
    fn each_task_has_its_ledger_type() {
        assert_eq!(
            StarterTask::CreateServer.ledger_type(),
            "starter:create_server"
        );
        assert_eq!(
            serde_json::to_string(&StarterTask::StartServer).unwrap(),
            "\"start_server\""
        );
        assert!(
            StarterTask::ALL
                .iter()
                .all(|task| task.ledger_type().starts_with(TYPE_PREFIX))
        );
    }
}
