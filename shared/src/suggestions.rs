//! plugged.host: the suggestions board (docs/features/suggestions.md): every account may post an
//! idea and vote for as many ideas as it likes, one vote per idea. Ideas are sorted by votes and
//! then by age; administrators move them through the statuses, write the changelog entry when an
//! idea ships and merge duplicates into the idea that stays, moving the votes with them.

use crate::models::Pagination;
use chrono::NaiveDateTime;
use serde::Serialize;
use serde_json::json;
use sqlx::Row;
use utoipa::ToSchema;

pub const UNDER_REVIEW: &str = "under_review";
pub const PLANNED: &str = "planned";
pub const IN_PROGRESS: &str = "in_progress";
pub const SHIPPED: &str = "shipped";
pub const DECLINED: &str = "declined";
/// A duplicate that was merged into another idea; not shown on the board.
pub const MERGED: &str = "merged";
pub const STATUSES: [&str; 6] = [
    UNDER_REVIEW,
    PLANNED,
    IN_PROGRESS,
    SHIPPED,
    DECLINED,
    MERGED,
];
/// The statuses an administrator picks; `merged` only comes from merging.
pub const EDITABLE_STATUSES: [&str; 5] = [UNDER_REVIEW, PLANNED, IN_PROGRESS, SHIPPED, DECLINED];

pub const MINECRAFT: &str = "minecraft";
pub const FIVEM: &str = "fivem";
pub const RUST: &str = "rust";
pub const TEAMSPEAK: &str = "teamspeak";
pub const OTHER: &str = "other";
pub const SERVER_TYPES: [&str; 5] = [MINECRAFT, FIVEM, RUST, TEAMSPEAK, OTHER];

pub const TITLE_MIN: usize = 3;
pub const TITLE_MAX: usize = 120;
pub const BODY_MIN: usize = 10;
pub const BODY_MAX: usize = 500;
pub const CHANGELOG_MAX: usize = 5000;

#[derive(Debug)]
pub enum SuggestionError {
    NotFound,
    Invalid(String),
    Database(anyhow::Error),
}

impl From<sqlx::Error> for SuggestionError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err.into())
    }
}

impl From<anyhow::Error> for SuggestionError {
    fn from(err: anyhow::Error) -> Self {
        Self::Database(err)
    }
}

impl SuggestionError {
    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "this suggestion does not exist".into(),
            Self::Invalid(message) => message.clone(),
            Self::Database(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::Database(_) => 500,
            Self::Invalid(_) => 400,
        }
    }

    pub fn response(self) -> crate::response::ApiResponse {
        let status = axum::http::StatusCode::from_u16(self.status())
            .unwrap_or(axum::http::StatusCode::BAD_REQUEST);

        match self {
            Self::Database(err) => crate::response::ApiResponse::from(err),
            err => crate::response::ApiResponse::error(&err.message()).with_status(status),
        }
    }
}

#[derive(ToSchema, Serialize, Clone)]
pub struct ApiSuggestionUser {
    pub uuid: uuid::Uuid,
    pub username: compact_str::CompactString,
}

#[derive(ToSchema, Serialize, Clone)]
pub struct ApiSuggestionRef {
    pub uuid: uuid::Uuid,
    pub title: compact_str::CompactString,
}

#[derive(ToSchema, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApiSuggestion {
    pub uuid: uuid::Uuid,
    pub title: compact_str::CompactString,
    pub body: String,
    pub server_type: compact_str::CompactString,
    pub status: compact_str::CompactString,
    /// What shipped, written by an administrator.
    pub changelog: Option<String>,
    pub votes: i64,
    /// Whether the reader voted for it.
    pub voted: bool,
    pub author: Option<ApiSuggestionUser>,
    pub merged_into: Option<ApiSuggestionRef>,
    pub created: chrono::DateTime<chrono::Utc>,
    pub updated: chrono::DateTime<chrono::Utc>,
}

fn utc(value: NaiveDateTime) -> chrono::DateTime<chrono::Utc> {
    value.and_utc()
}

const SELECT: &str = "SELECT suggestions.uuid, suggestions.title, suggestions.body,
        suggestions.server_type, suggestions.status, suggestions.changelog,
        suggestions.created, suggestions.updated,
        authors.uuid AS author_uuid, authors.username AS author_username,
        merged.uuid AS merged_uuid, merged.title AS merged_title,
        (SELECT COUNT(*) FROM suggestion_votes WHERE suggestion_votes.suggestion_uuid = suggestions.uuid) AS votes,
        EXISTS(SELECT 1 FROM suggestion_votes WHERE suggestion_votes.suggestion_uuid = suggestions.uuid
            AND suggestion_votes.user_uuid = $1) AS voted
    FROM suggestions
    LEFT JOIN users AS authors ON authors.uuid = suggestions.user_uuid
    LEFT JOIN suggestions AS merged ON merged.uuid = suggestions.merged_into";

fn map(row: &sqlx::postgres::PgRow) -> Result<ApiSuggestion, sqlx::Error> {
    let author_uuid: Option<uuid::Uuid> = row.try_get("author_uuid")?;
    let author_username: Option<compact_str::CompactString> = row.try_get("author_username")?;
    let merged_uuid: Option<uuid::Uuid> = row.try_get("merged_uuid")?;
    let merged_title: Option<compact_str::CompactString> = row.try_get("merged_title")?;
    let created: NaiveDateTime = row.try_get("created")?;
    let updated: NaiveDateTime = row.try_get("updated")?;

    Ok(ApiSuggestion {
        uuid: row.try_get("uuid")?,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        server_type: row.try_get("server_type")?,
        status: row.try_get("status")?,
        changelog: row.try_get("changelog")?,
        votes: row.try_get("votes")?,
        voted: row.try_get("voted")?,
        author: author_uuid
            .zip(author_username)
            .map(|(uuid, username)| ApiSuggestionUser { uuid, username }),
        merged_into: merged_uuid
            .zip(merged_title)
            .map(|(uuid, title)| ApiSuggestionRef { uuid, title }),
        created: utc(created),
        updated: utc(updated),
    })
}

/// The board: merged duplicates are left out, the rest by votes and then newest.
pub async fn list(
    database: &crate::database::Database,
    viewer_uuid: uuid::Uuid,
) -> Result<Vec<ApiSuggestion>, sqlx::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE suggestions.status <> 'merged'
        ORDER BY votes DESC, suggestions.created DESC"
    )))
    .bind(viewer_uuid)
    .fetch_all(database.read())
    .await?;

    rows.iter().map(map).collect()
}

/// One idea, for the account that just voted or created it.
pub async fn view(
    database: &crate::database::Database,
    viewer_uuid: uuid::Uuid,
    suggestion_uuid: uuid::Uuid,
) -> Result<Option<ApiSuggestion>, sqlx::Error> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE suggestions.uuid = $2"
    )))
    .bind(viewer_uuid)
    .bind(suggestion_uuid)
    .fetch_optional(database.read())
    .await?;

    row.as_ref().map(map).transpose()
}

pub struct Create {
    pub title: String,
    pub body: String,
    pub server_type: String,
}

/// Posts an idea; administrators are told about it.
pub async fn create(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    username: &str,
    request: Create,
) -> Result<uuid::Uuid, SuggestionError> {
    let title = request.title.trim();
    if title.chars().count() < TITLE_MIN || title.chars().count() > TITLE_MAX {
        return Err(SuggestionError::Invalid(format!(
            "a title is {TITLE_MIN} to {TITLE_MAX} characters"
        )));
    }
    let body = request.body.trim();
    if body.chars().count() < BODY_MIN || body.chars().count() > BODY_MAX {
        return Err(SuggestionError::Invalid(format!(
            "a description is {BODY_MIN} to {BODY_MAX} characters"
        )));
    }
    if !SERVER_TYPES.contains(&request.server_type.as_str()) {
        return Err(SuggestionError::Invalid("unknown server type".into()));
    }

    let suggestion_uuid = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO suggestions (user_uuid, title, body, server_type)
        VALUES ($1, $2, $3, $4) RETURNING uuid",
    )
    .bind(user_uuid)
    .bind(title)
    .bind(body)
    .bind(&request.server_type)
    .fetch_one(state.database.write())
    .await?;

    crate::notifications::send_admins(
        state,
        "suggestions.manage",
        None,
        crate::notifications::ADMIN_SUGGESTION_CREATED,
        json!({ "title": title, "user": username }),
    )
    .await;

    Ok(suggestion_uuid)
}

/// One vote per account: voting again takes the vote back. Returns the idea as it is now.
pub async fn toggle_vote(
    state: &crate::State,
    viewer_uuid: uuid::Uuid,
    suggestion_uuid: uuid::Uuid,
) -> Result<ApiSuggestion, SuggestionError> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM suggestions WHERE uuid = $1")
            .bind(suggestion_uuid)
            .fetch_optional(state.database.read())
            .await?;
    let Some(status) = status else {
        return Err(SuggestionError::NotFound);
    };
    if status.as_str() == MERGED {
        return Err(SuggestionError::Invalid(
            "this suggestion was merged into another one".into(),
        ));
    }
    let voted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM suggestion_votes WHERE suggestion_uuid = $1 AND user_uuid = $2)",
    )
    .bind(suggestion_uuid)
    .bind(viewer_uuid)
    .fetch_one(state.database.read())
    .await?;

    if voted {
        sqlx::query("DELETE FROM suggestion_votes WHERE suggestion_uuid = $1 AND user_uuid = $2")
            .bind(suggestion_uuid)
            .bind(viewer_uuid)
            .execute(state.database.write())
            .await?;
    } else {
        // plugged.host: the central service recognises a second account on the same person. A
        // failed check is no verdict and never blocks the vote.
        if crate::noprofit::alt_limited(state, viewer_uuid).await {
            return Err(SuggestionError::Invalid(
                "your vote already counted for this person".into(),
            ));
        }

        sqlx::query(
            "INSERT INTO suggestion_votes (suggestion_uuid, user_uuid) VALUES ($1, $2)
            ON CONFLICT DO NOTHING",
        )
        .bind(suggestion_uuid)
        .bind(viewer_uuid)
        .execute(state.database.write())
        .await?;
        crate::noprofit::alt_signal(
            state,
            viewer_uuid,
            "suggestion_vote",
            json!({ "suggestion_uuid": suggestion_uuid }),
        )
        .await;
    }

    view(&state.database, viewer_uuid, suggestion_uuid)
        .await?
        .ok_or(SuggestionError::NotFound)
}

pub struct AdminFilter<'a> {
    pub status: Option<&'a str>,
    pub server_type: Option<&'a str>,
    pub search: Option<&'a str>,
}

/// Every idea, for Admin › Suggestions.
pub async fn admin_list(
    state: &crate::State,
    viewer_uuid: uuid::Uuid,
    filter: AdminFilter<'_>,
    page: i64,
    per_page: i64,
) -> Result<Pagination<ApiSuggestion>, anyhow::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{SELECT}
        WHERE ($2::text IS NULL OR suggestions.status = $2)
            AND ($3::text IS NULL OR suggestions.server_type = $3)
            AND ($4::text IS NULL OR suggestions.title ILIKE '%' || $4 || '%'
                OR suggestions.body ILIKE '%' || $4 || '%'
                OR authors.username ILIKE '%' || $4 || '%')
        ORDER BY
            CASE suggestions.status
                WHEN 'under_review' THEN 0
                WHEN 'planned' THEN 1
                WHEN 'in_progress' THEN 2
                WHEN 'shipped' THEN 3
                WHEN 'declined' THEN 4
                ELSE 5
            END,
            suggestions.created DESC
        LIMIT $5 OFFSET $6"
    )))
    .bind(viewer_uuid)
    .bind(filter.status)
    .bind(filter.server_type)
    .bind(filter.search)
    .bind(per_page)
    .bind((page - 1) * per_page)
    .fetch_all(state.database.read())
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM suggestions
        LEFT JOIN users AS authors ON authors.uuid = suggestions.user_uuid
        WHERE ($1::text IS NULL OR suggestions.status = $1)
            AND ($2::text IS NULL OR suggestions.server_type = $2)
            AND ($3::text IS NULL OR suggestions.title ILIKE '%' || $3 || '%'
                OR suggestions.body ILIKE '%' || $3 || '%'
                OR authors.username ILIKE '%' || $3 || '%')",
    )
    .bind(filter.status)
    .bind(filter.server_type)
    .bind(filter.search)
    .fetch_one(state.database.read())
    .await?;

    Ok(Pagination {
        total,
        per_page,
        page,
        data: rows.iter().map(map).collect::<Result<_, _>>()?,
    })
}

/// An administrator moves an idea's status and writes its changelog entry.
pub async fn update(
    state: &crate::State,
    suggestion_uuid: uuid::Uuid,
    status: &str,
    changelog: Option<&str>,
) -> Result<(), SuggestionError> {
    if !EDITABLE_STATUSES.contains(&status) {
        return Err(SuggestionError::Invalid("unknown status".into()));
    }
    let changelog = changelog.map(str::trim).filter(|entry| !entry.is_empty());
    if changelog.is_some_and(|entry| entry.chars().count() > CHANGELOG_MAX) {
        return Err(SuggestionError::Invalid(format!(
            "a changelog entry is at most {CHANGELOG_MAX} characters"
        )));
    }

    let changed = sqlx::query(
        "UPDATE suggestions SET status = $2, changelog = $3, updated = NOW()
        WHERE uuid = $1 AND status <> 'merged'",
    )
    .bind(suggestion_uuid)
    .bind(status)
    .bind(changelog)
    .execute(state.database.write())
    .await?
    .rows_affected();
    if changed == 0 {
        return Err(SuggestionError::NotFound);
    }

    Ok(())
}

/// Merges `source` into `target`: the votes move, the source is marked merged and hidden.
pub async fn merge(
    state: &crate::State,
    source_uuid: uuid::Uuid,
    target_uuid: uuid::Uuid,
) -> Result<(), SuggestionError> {
    if source_uuid == target_uuid {
        return Err(SuggestionError::Invalid(
            "a suggestion cannot be merged into itself".into(),
        ));
    }

    let mut transaction = state.database.write().begin().await?;
    let target: Option<String> = sqlx::query_scalar(
        "SELECT title FROM suggestions WHERE uuid = $1 AND status <> 'merged' FOR UPDATE",
    )
    .bind(target_uuid)
    .fetch_optional(&mut *transaction)
    .await?;
    if target.is_none() {
        return Err(SuggestionError::Invalid(
            "the suggestion to merge into does not exist".into(),
        ));
    }
    let source: Option<String> =
        sqlx::query_scalar("SELECT title FROM suggestions WHERE uuid = $1 FOR UPDATE")
            .bind(source_uuid)
            .fetch_optional(&mut *transaction)
            .await?;
    if source.is_none() {
        return Err(SuggestionError::NotFound);
    }

    sqlx::query(
        "INSERT INTO suggestion_votes (suggestion_uuid, user_uuid, created)
        SELECT $2, user_uuid, created FROM suggestion_votes WHERE suggestion_uuid = $1
        ON CONFLICT DO NOTHING",
    )
    .bind(source_uuid)
    .bind(target_uuid)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("DELETE FROM suggestion_votes WHERE suggestion_uuid = $1")
        .bind(source_uuid)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "UPDATE suggestions SET merged_into = $2, status = 'merged', updated = NOW() WHERE uuid = $1",
    )
    .bind(source_uuid)
    .bind(target_uuid)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(())
}

/// Deletes an idea with its votes.
pub async fn delete(
    state: &crate::State,
    suggestion_uuid: uuid::Uuid,
) -> Result<(), SuggestionError> {
    let changed = sqlx::query("DELETE FROM suggestions WHERE uuid = $1")
        .bind(suggestion_uuid)
        .execute(state.database.write())
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(SuggestionError::NotFound);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_are_the_documented_ones() {
        assert_eq!(STATUSES.len(), 6);
        assert_eq!(
            EDITABLE_STATUSES,
            [
                "under_review",
                "planned",
                "in_progress",
                "shipped",
                "declined"
            ]
        );
        assert!(STATUSES.contains(&MERGED));
        assert!(!EDITABLE_STATUSES.contains(&MERGED));
    }

    #[test]
    fn server_types_are_offered() {
        assert_eq!(
            SERVER_TYPES,
            ["minecraft", "fivem", "rust", "teamspeak", "other"]
        );
    }
}
