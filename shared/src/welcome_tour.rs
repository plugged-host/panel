//! plugged.host: where each user is in the welcome tour (docs/features/welcome-tour.md).
//!
//! The tour has two parts because it runs on the real pages: the first on the server list, the
//! second on a server's console, which a new account only reaches once it has a server. The state
//! is kept per tour version: raising the version in Admin › Settings › Welcome Tour shows it to
//! everyone again.

use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

/// A user's progress through the current tour version.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TourState {
    pub version: i32,
    /// The step of the first part to show next (1-based).
    pub step: i32,
    pub server_step: i32,
    pub completed: Option<chrono::NaiveDateTime>,
    pub skipped: Option<chrono::NaiveDateTime>,
    pub server_done: Option<chrono::NaiveDateTime>,
}

impl TourState {
    fn fresh(version: i32) -> Self {
        Self {
            version,
            step: 1,
            server_step: 1,
            ..Default::default()
        }
    }
}

/// What the frontend shows.
#[derive(ToSchema, Serialize, Debug, PartialEq)]
pub struct ApiTour {
    /// The first part runs on the server list.
    pub panel_active: bool,
    /// The second part runs on the next console the user opens.
    pub server_active: bool,
    pub step: i32,
    pub server_step: i32,
}

#[derive(ToSchema, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TourAction {
    /// The first part moved to `step`.
    Advance,
    /// The second part moved to `step`.
    AdvanceServer,
    /// The first part is finished; the second one waits for a console.
    Complete,
    /// "Skip tour": both parts end.
    Skip,
    /// The second part is over, finished or closed alike.
    CompleteServer,
    /// "Replay the welcome tour": both parts from the start.
    Restart,
}

/// The state as the current version sees it: one written for an older version counts as none.
pub fn current(stored: Option<TourState>, version: i32) -> TourState {
    stored
        .filter(|state| state.version == version)
        .unwrap_or_else(|| TourState::fresh(version))
}

pub fn view(enabled: bool, state: &TourState) -> ApiTour {
    ApiTour {
        panel_active: enabled && state.completed.is_none() && state.skipped.is_none(),
        server_active: enabled
            && state.completed.is_some()
            && state.skipped.is_none()
            && state.server_done.is_none(),
        step: state.step.max(1),
        server_step: state.server_step.max(1),
    }
}

pub fn apply(
    mut state: TourState,
    action: TourAction,
    step: i32,
    now: chrono::NaiveDateTime,
) -> TourState {
    match action {
        TourAction::Advance => state.step = step.max(1),
        TourAction::AdvanceServer => state.server_step = step.max(1),
        TourAction::Complete => {
            state.completed = Some(now);
            state.step = 1;
        }
        TourAction::Skip => {
            state.skipped = Some(now);
            state.server_done = state.server_done.or(Some(now));
        }
        TourAction::CompleteServer => {
            state.server_done = Some(now);
            state.server_step = 1;
        }
        TourAction::Restart => state = TourState::fresh(state.version),
    }

    state
}

async fn stored(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Result<Option<TourState>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT version, step, server_step, completed, skipped, server_done
        FROM user_welcome_tours WHERE user_uuid = $1",
    )
    .bind(user_uuid)
    .fetch_optional(database.write())
    .await?;

    row.map(|row| {
        Ok(TourState {
            version: row.try_get("version")?,
            step: row.try_get("step")?,
            server_step: row.try_get("server_step")?,
            completed: row.try_get("completed")?,
            skipped: row.try_get("skipped")?,
            server_done: row.try_get("server_done")?,
        })
    })
    .transpose()
}

/// The user's tour now.
pub async fn get(state: &crate::State, user_uuid: uuid::Uuid) -> Result<ApiTour, anyhow::Error> {
    let (enabled, version) = {
        let settings = state.settings.get().await?;
        (settings.tour.enabled, settings.tour.version)
    };

    let tour = current(stored(&state.database, user_uuid).await?, version);

    Ok(view(enabled, &tour))
}

/// Records a step, the end of a part, a skip or a replay.
pub async fn act(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    action: TourAction,
    step: i32,
) -> Result<ApiTour, anyhow::Error> {
    let (enabled, version) = {
        let settings = state.settings.get().await?;
        (settings.tour.enabled, settings.tour.version)
    };

    let tour = apply(
        current(stored(&state.database, user_uuid).await?, version),
        action,
        step,
        chrono::Utc::now().naive_utc(),
    );

    sqlx::query(
        "INSERT INTO user_welcome_tours
            (user_uuid, version, step, server_step, completed, skipped, server_done, updated)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
        ON CONFLICT (user_uuid) DO UPDATE SET
            version = EXCLUDED.version,
            step = EXCLUDED.step,
            server_step = EXCLUDED.server_step,
            completed = EXCLUDED.completed,
            skipped = EXCLUDED.skipped,
            server_done = EXCLUDED.server_done,
            updated = NOW()",
    )
    .bind(user_uuid)
    .bind(tour.version)
    .bind(tour.step)
    .bind(tour.server_step)
    .bind(tour.completed)
    .bind(tour.skipped)
    .bind(tour.server_done)
    .execute(state.database.write())
    .await?;

    Ok(view(enabled, &tour))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 23)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    #[test]
    fn the_second_part_follows_the_first() {
        let state = current(None, 1);
        assert!(view(true, &state).panel_active);
        assert!(!view(true, &state).server_active);
        assert!(!view(false, &state).panel_active);

        let state = apply(state, TourAction::Advance, 3, now());
        assert_eq!(view(true, &state).step, 3);

        let state = apply(state, TourAction::Complete, 1, now());
        let shown = view(true, &state);
        assert!(!shown.panel_active && shown.server_active);
        assert_eq!(shown.step, 1);

        let state = apply(state, TourAction::CompleteServer, 1, now());
        let shown = view(true, &state);
        assert!(!shown.panel_active && !shown.server_active);
    }

    #[test]
    fn skipping_ends_both_parts_and_a_replay_starts_over() {
        let state = apply(current(None, 1), TourAction::Skip, 1, now());
        let shown = view(true, &state);
        assert!(!shown.panel_active && !shown.server_active);

        let state = apply(state, TourAction::Restart, 1, now());
        assert!(view(true, &state).panel_active);
        assert_eq!(state.version, 1);
    }

    #[test]
    fn a_new_version_shows_the_tour_again() {
        let seen = apply(current(None, 1), TourAction::Skip, 1, now());
        assert_eq!(current(Some(seen.clone()), 1), seen);
        assert!(view(true, &current(Some(seen), 2)).panel_active);
    }
}
