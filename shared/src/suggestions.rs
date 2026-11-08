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
pub const BODY_MAX: usize = 5000;
pub const MESSAGE_MAX: usize = 5000;
pub const CHANGELOG_MAX: usize = 5000;

/// A comment by the person who wrote it.
pub const AUTHOR_USER: &str = "user";
/// A reply by a staff member.
pub const AUTHOR_ADMIN: &str = "admin";

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
    /// The profile badge shown next to the name (shared/src/badges.rs).
    pub badge: Option<compact_str::CompactString>,
}

#[derive(ToSchema, Serialize, Clone)]
pub struct ApiSuggestionRef {
    pub uuid: uuid::Uuid,
    pub title: compact_str::CompactString,
}

#[derive(ToSchema, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApiSuggestionAttachment {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub mime: compact_str::CompactString,
    pub size: i64,
    pub url: String,
    pub created: chrono::DateTime<chrono::Utc>,
}

#[derive(ToSchema, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApiSuggestionMessage {
    pub uuid: uuid::Uuid,
    pub author: Option<ApiSuggestionUser>,
    /// `user` or `admin`.
    pub author_type: compact_str::CompactString,
    pub body: String,
    pub created: chrono::DateTime<chrono::Utc>,
}

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiSuggestionThread {
    pub suggestion: ApiSuggestion,
    pub messages: Vec<ApiSuggestionMessage>,
    pub attachments: Vec<ApiSuggestionAttachment>,
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
    /// How many comments the discussion has.
    pub comments: i64,
    /// How many images and files are attached.
    pub attachments: i64,
    /// The first image, shown as the board card's cover.
    pub preview_url: Option<String>,
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
        authors.uuid AS author_uuid, authors.username AS author_username, authors.badge AS author_badge,
        merged.uuid AS merged_uuid, merged.title AS merged_title,
        (SELECT COUNT(*) FROM suggestion_votes WHERE suggestion_votes.suggestion_uuid = suggestions.uuid) AS votes,
        EXISTS(SELECT 1 FROM suggestion_votes WHERE suggestion_votes.suggestion_uuid = suggestions.uuid
            AND suggestion_votes.user_uuid = $1) AS voted,
        (SELECT COUNT(*) FROM suggestion_messages WHERE suggestion_messages.suggestion_uuid = suggestions.uuid) AS comments,
        (SELECT COUNT(*) FROM suggestion_attachments WHERE suggestion_attachments.suggestion_uuid = suggestions.uuid) AS attachments
    FROM suggestions
    LEFT JOIN users AS authors ON authors.uuid = suggestions.user_uuid
    LEFT JOIN suggestions AS merged ON merged.uuid = suggestions.merged_into";

fn map(row: &sqlx::postgres::PgRow) -> Result<ApiSuggestion, sqlx::Error> {
    let author_uuid: Option<uuid::Uuid> = row.try_get("author_uuid")?;
    let author_username: Option<compact_str::CompactString> = row.try_get("author_username")?;
    let author_badge: Option<compact_str::CompactString> = row.try_get("author_badge")?;
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
        comments: row.try_get("comments")?,
        attachments: row.try_get("attachments")?,
        preview_url: None,
        author: author_uuid
            .zip(author_username)
            .map(|(uuid, username)| ApiSuggestionUser {
                uuid,
                username,
                badge: author_badge,
            }),
        merged_into: merged_uuid
            .zip(merged_title)
            .map(|(uuid, title)| ApiSuggestionRef { uuid, title }),
        created: utc(created),
        updated: utc(updated),
    })
}

/// The board: merged duplicates are left out, the rest by votes and then newest.
pub async fn list(
    state: &crate::State,
    viewer_uuid: uuid::Uuid,
) -> anyhow::Result<Vec<ApiSuggestion>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE suggestions.status <> 'merged'
        ORDER BY votes DESC, suggestions.created DESC"
    )))
    .bind(viewer_uuid)
    .fetch_all(state.database.read())
    .await?;

    let mut suggestions: Vec<ApiSuggestion> = rows.iter().map(map).collect::<Result<_, _>>()?;
    with_previews(state, &mut suggestions).await?;
    Ok(suggestions)
}

/// The first image of each idea, for the board's covers.
async fn with_previews(
    state: &crate::State,
    suggestions: &mut [ApiSuggestion],
) -> Result<(), anyhow::Error> {
    let uuids: Vec<uuid::Uuid> = suggestions
        .iter()
        .map(|suggestion| suggestion.uuid)
        .collect();
    if uuids.is_empty() {
        return Ok(());
    }

    let rows = sqlx::query(
        "SELECT suggestion_uuid, path FROM suggestion_attachments
        WHERE suggestion_uuid = ANY($1) AND mime LIKE 'image/%'
        ORDER BY created, uuid",
    )
    .bind(&uuids)
    .fetch_all(state.database.read())
    .await?;
    let urls = state.storage.retrieve_urls().await?;
    let mut first: std::collections::HashMap<uuid::Uuid, String> = std::collections::HashMap::new();
    for row in &rows {
        first
            .entry(row.try_get("suggestion_uuid")?)
            .or_insert(row.try_get("path")?);
    }
    for suggestion in suggestions {
        if let Some(path) = first.get(&suggestion.uuid) {
            suggestion.preview_url = Some(urls.get_url(path));
        }
    }
    Ok(())
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

/// One idea with its discussion and attachments.
pub async fn thread(
    state: &crate::State,
    viewer_uuid: uuid::Uuid,
    suggestion_uuid: uuid::Uuid,
) -> Result<Option<ApiSuggestionThread>, anyhow::Error> {
    let Some(suggestion) = view(&state.database, viewer_uuid, suggestion_uuid).await? else {
        return Ok(None);
    };
    Ok(Some(ApiSuggestionThread {
        suggestion,
        messages: messages(&state.database, suggestion_uuid).await?,
        attachments: attachments(state, suggestion_uuid).await?,
    }))
}

/// The discussion, oldest first.
pub async fn messages(
    database: &crate::database::Database,
    suggestion_uuid: uuid::Uuid,
) -> Result<Vec<ApiSuggestionMessage>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT suggestion_messages.uuid, suggestion_messages.author_type,
            suggestion_messages.body, suggestion_messages.created,
            users.uuid AS author_uuid, users.username AS author_username, users.badge AS author_badge
        FROM suggestion_messages
        LEFT JOIN users ON users.uuid = suggestion_messages.author_uuid
        WHERE suggestion_messages.suggestion_uuid = $1
        ORDER BY suggestion_messages.created, suggestion_messages.uuid",
    )
    .bind(suggestion_uuid)
    .fetch_all(database.read())
    .await?;

    rows.iter()
        .map(|row| {
            let author_uuid: Option<uuid::Uuid> = row.try_get("author_uuid")?;
            let author_username: Option<compact_str::CompactString> =
                row.try_get("author_username")?;
            let author_badge: Option<compact_str::CompactString> = row.try_get("author_badge")?;
            Ok(ApiSuggestionMessage {
                uuid: row.try_get("uuid")?,
                author: author_uuid.zip(author_username).map(|(uuid, username)| {
                    ApiSuggestionUser {
                        uuid,
                        username,
                        badge: author_badge,
                    }
                }),
                author_type: row.try_get("author_type")?,
                body: row.try_get("body")?,
                created: utc(row.try_get("created")?),
            })
        })
        .collect()
}

/// The idea's images and files, with the URL the panel serves them from.
pub async fn attachments(
    state: &crate::State,
    suggestion_uuid: uuid::Uuid,
) -> Result<Vec<ApiSuggestionAttachment>, anyhow::Error> {
    let rows = sqlx::query(
        "SELECT uuid, name, mime, size, path, created FROM suggestion_attachments
        WHERE suggestion_uuid = $1 ORDER BY created, uuid",
    )
    .bind(suggestion_uuid)
    .fetch_all(state.database.read())
    .await?;
    let urls = state.storage.retrieve_urls().await?;

    rows.iter()
        .map(|row| {
            let path: String = row.try_get("path")?;
            Ok(ApiSuggestionAttachment {
                uuid: row.try_get("uuid")?,
                name: row.try_get("name")?,
                mime: row.try_get("mime")?,
                size: row.try_get("size")?,
                url: urls.get_url(&path),
                created: utc(row.try_get("created")?),
            })
        })
        .collect()
}

/// Adds a comment or a staff reply to an idea.
pub async fn comment(
    state: &crate::State,
    suggestion_uuid: uuid::Uuid,
    author_uuid: uuid::Uuid,
    author_type: &str,
    body: &str,
) -> Result<ApiSuggestionThread, SuggestionError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(SuggestionError::Invalid("a comment cannot be empty".into()));
    }
    if body.chars().count() > MESSAGE_MAX {
        return Err(SuggestionError::Invalid(format!(
            "a comment is at most {MESSAGE_MAX} characters"
        )));
    }
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM suggestions WHERE uuid = $1")
            .bind(suggestion_uuid)
            .fetch_optional(state.database.read())
            .await?;
    let Some(status) = status else {
        return Err(SuggestionError::NotFound);
    };
    if status == MERGED {
        return Err(SuggestionError::Invalid(
            "this suggestion was merged into another one".into(),
        ));
    }

    sqlx::query(
        "INSERT INTO suggestion_messages (suggestion_uuid, author_uuid, author_type, body)
        VALUES ($1, $2, $3, $4)",
    )
    .bind(suggestion_uuid)
    .bind(author_uuid)
    .bind(author_type)
    .bind(body)
    .execute(state.database.write())
    .await?;
    sqlx::query("UPDATE suggestions SET updated = NOW() WHERE uuid = $1")
        .bind(suggestion_uuid)
        .execute(state.database.write())
        .await?;

    thread(state, author_uuid, suggestion_uuid)
        .await?
        .ok_or(SuggestionError::NotFound)
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

    let mut data: Vec<ApiSuggestion> = rows.iter().map(map).collect::<Result<_, _>>()?;
    with_previews(state, &mut data).await?;
    Ok(Pagination {
        total,
        per_page,
        page,
        data,
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
    let paths: Vec<String> =
        sqlx::query_scalar("SELECT path FROM suggestion_attachments WHERE suggestion_uuid = $1")
            .bind(suggestion_uuid)
            .fetch_all(state.database.read())
            .await?;
    let changed = sqlx::query("DELETE FROM suggestions WHERE uuid = $1")
        .bind(suggestion_uuid)
        .execute(state.database.write())
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(SuggestionError::NotFound);
    }
    for path in paths {
        if let Err(error) = state.storage.remove(Some(path)).await {
            tracing::warn!(suggestion = %suggestion_uuid, "failed to remove a suggestion attachment: {error:#}");
        }
    }

    Ok(())
}

/// Deletes an idea the account owns (the admin delete is unrestricted).
pub async fn delete_owned(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    suggestion_uuid: uuid::Uuid,
) -> Result<(), SuggestionError> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM suggestions WHERE uuid = $1 AND user_uuid = $2)",
    )
    .bind(suggestion_uuid)
    .bind(user_uuid)
    .fetch_one(state.database.read())
    .await?;
    if !owned {
        return Err(SuggestionError::NotFound);
    }
    delete(state, suggestion_uuid).await
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
