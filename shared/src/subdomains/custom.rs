//! plugged.host: a server's own domain (docs/features/subdomains.md#custom-domains). The owner
//! points a CNAME at the server's panel subdomain (or address when it has none) and proves the
//! domain with a TXT record; the panel asks Cloudflare's DNS-over-HTTPS resolver, never a data
//! plane. Only a verified domain shows as active. The panel writes no records for a custom domain:
//! it belongs to the person, and the panel only checks where it points.

use super::{Target, target, valid_domain};
use crate::{models::server::Server, prelude::SqlxErrorExt};
use chrono::NaiveDateTime;
use rand::distr::SampleString;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

const DOH: &str = "https://cloudflare-dns.com/dns-query";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug)]
pub enum CustomDomainError {
    /// Subdomains (and with them custom domains) are off.
    Off,
    /// The server has no address a custom domain could follow.
    NoAddress,
    InvalidName,
    /// A domain of Admin › Domains, or the panel's own host.
    PanelDomain,
    Exists,
    NotFound,
    /// The records do not point at the target or the token is missing.
    NotVerified(String),
    Dns(String),
    Database(anyhow::Error),
}

impl From<sqlx::Error> for CustomDomainError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err.into())
    }
}

impl From<anyhow::Error> for CustomDomainError {
    fn from(err: anyhow::Error) -> Self {
        Self::Database(err)
    }
}

impl CustomDomainError {
    pub fn message(&self) -> String {
        match self {
            Self::Off => "subdomains are not set up on this panel".into(),
            Self::NoAddress => "the server has no address a custom domain could point at".into(),
            Self::InvalidName => "this is not a domain name".into(),
            Self::PanelDomain => "this domain belongs to the panel".into(),
            Self::Exists => "this domain is already attached".into(),
            Self::NotFound => "this custom domain does not exist".into(),
            Self::NotVerified(message) => message.clone(),
            Self::Dns(message) => format!("DNS lookup failed: {message}"),
            Self::Database(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::Exists => 409,
            Self::NotVerified(_) => 422,
            Self::Dns(_) => 502,
            Self::Database(_) => 500,
            _ => 400,
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
pub struct ApiCustomDomain {
    pub uuid: uuid::Uuid,
    pub domain: compact_str::CompactString,
    /// What the CNAME points at: the server's panel subdomain, or its address.
    pub target: compact_str::CompactString,
    /// The value of the TXT record at `verification_name`.
    pub verification_token: compact_str::CompactString,
    pub verification_name: compact_str::CompactString,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub struct CustomDomain {
    pub uuid: uuid::Uuid,
    pub server_uuid: uuid::Uuid,
    pub domain: compact_str::CompactString,
    pub target: compact_str::CompactString,
    pub verification_token: compact_str::CompactString,
    pub verified_at: Option<NaiveDateTime>,
    pub created: NaiveDateTime,
}

impl CustomDomain {
    fn map(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            uuid: row.try_get("uuid")?,
            server_uuid: row.try_get("server_uuid")?,
            domain: row.try_get("domain")?,
            target: row.try_get("target")?,
            verification_token: row.try_get("verification_token")?,
            verified_at: row.try_get("verified_at")?,
            created: row.try_get("created")?,
        })
    }

    pub fn into_api(self) -> ApiCustomDomain {
        let verification_name = format!("_panel-verify.{}", self.domain);

        ApiCustomDomain {
            uuid: self.uuid,
            domain: self.domain,
            target: self.target,
            verification_token: self.verification_token,
            verification_name: verification_name.into(),
            verified_at: self.verified_at.map(|value| value.and_utc()),
            created: self.created.and_utc(),
        }
    }
}

const SELECT: &str = "SELECT uuid, server_uuid, domain, target, verification_token,
        verified_at, created FROM server_custom_domains";

pub async fn all_by_server_uuid(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<Vec<CustomDomain>, sqlx::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE server_uuid = $1 ORDER BY created"
    )))
    .bind(server_uuid)
    .fetch_all(database.read())
    .await?;

    rows.iter().map(CustomDomain::map).collect()
}

pub async fn by_uuid(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
    uuid: uuid::Uuid,
) -> Result<Option<CustomDomain>, sqlx::Error> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE server_uuid = $1 AND uuid = $2"
    )))
    .bind(server_uuid)
    .bind(uuid)
    .fetch_optional(database.read())
    .await?;

    row.as_ref().map(CustomDomain::map).transpose()
}

/// What a custom domain should point at: the server's first panel subdomain, else its address.
pub async fn target_for(
    state: &crate::State,
    server: &Server,
) -> Result<Option<String>, anyhow::Error> {
    if let Some(subdomain) = super::Subdomain::all_by_server_uuid(&state.database, server.uuid)
        .await?
        .into_iter()
        .next()
    {
        return Ok(Some(subdomain.fqdn()));
    }

    let Some(target) = target(state, server).await? else {
        return Ok(None);
    };

    Ok(Some(match target {
        Target {
            srv_target: Some(host),
            ..
        } => host,
        Target { ip, .. } => ip.to_string(),
    }))
}

/// Whether `domain` is the panel's or one of the subdomain domains: those may not be attached.
async fn is_panel_domain(state: &crate::State, domain: &str) -> Result<bool, anyhow::Error> {
    let mut hosts: Vec<String> = {
        let rows = sqlx::query_scalar::<_, String>("SELECT name FROM domains")
            .fetch_all(state.database.read())
            .await?;
        rows
    };
    {
        let settings = state.settings.get().await?;
        for url in settings.app.urls() {
            if let Ok(url) = reqwest::Url::parse(url)
                && let Some(host) = url.host_str()
            {
                hosts.push(host.to_string());
            }
        }
    }

    Ok(hosts.into_iter().any(|host| {
        let host = host.trim_end_matches('.').to_lowercase();
        domain == host || domain.ends_with(&format!(".{host}"))
    }))
}

/// Attaches a domain, unverified until [`verify`] finds its records.
pub async fn add(
    state: &crate::State,
    server: &Server,
    domain: &str,
) -> Result<CustomDomain, CustomDomainError> {
    if !state.settings.get().await?.subdomains.active() {
        return Err(CustomDomainError::Off);
    }

    let domain = domain.trim().trim_end_matches('.').to_lowercase();
    if !valid_domain(&domain) {
        return Err(CustomDomainError::InvalidName);
    }
    if is_panel_domain(state, &domain).await? {
        return Err(CustomDomainError::PanelDomain);
    }
    let target = target_for(state, server)
        .await?
        .ok_or(CustomDomainError::NoAddress)?;
    let token = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 32);

    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO server_custom_domains (server_uuid, domain, target, verification_token)
        VALUES ($1, $2, $3, $4)
        RETURNING uuid, server_uuid, domain, target, verification_token, verified_at, created"
    )))
    .bind(server.uuid)
    .bind(&domain)
    .bind(&target)
    .bind(&token)
    .fetch_one(state.database.write())
    .await;
    let row = match row {
        Ok(row) => row,
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            return Err(CustomDomainError::Exists);
        }
        Err(err) => return Err(err.into()),
    };

    Ok(CustomDomain::map(&row)?)
}

#[derive(Deserialize)]
struct Doh {
    #[serde(default, rename = "Status")]
    status: i64,
    #[serde(default, rename = "Answer")]
    answer: Vec<DohAnswer>,
}

#[derive(Deserialize)]
struct DohAnswer {
    #[serde(default, rename = "type")]
    kind: u16,
    #[serde(default)]
    data: String,
}

async fn lookup(
    state: &crate::State,
    name: &str,
    kind: &str,
) -> Result<Vec<DohAnswer>, CustomDomainError> {
    let response = state
        .client
        .get(DOH)
        .query(&[("name", name), ("type", kind)])
        .header("accept", "application/dns-json")
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|err| CustomDomainError::Dns(err.to_string()))?;
    let status = response.status();
    let answer: Doh = response.json().await.map_err(|_| {
        CustomDomainError::Dns(format!("the resolver answered {status} without its JSON"))
    })?;

    // 0: NOERROR, 3: NXDOMAIN; anything else is a resolver problem, not a missing record
    if answer.status != 0 && answer.status != 3 {
        return Err(CustomDomainError::Dns(format!(
            "the resolver answered status {}",
            answer.status
        )));
    }

    Ok(answer.answer)
}

fn normalize(value: &str) -> String {
    value.trim().trim_end_matches('.').to_lowercase()
}

/// Checks the domain's records now: the TXT token at `_panel-verify.<domain>` and the CNAME (or,
/// for a target that is an IP address, the A/AAAA record).
pub async fn verify(
    state: &crate::State,
    custom: CustomDomain,
) -> Result<CustomDomain, CustomDomainError> {
    let verification_name = format!("_panel-verify.{}", custom.domain);
    let txt = lookup(state, &verification_name, "TXT").await?;
    let expected = format!("panel-verify={}", custom.verification_token);
    let token_found = txt.iter().any(|answer| {
        answer.kind == 16 && normalize(&answer.data.replace('"', "")) == normalize(&expected)
    });
    if !token_found {
        return Err(CustomDomainError::NotVerified(format!(
            "add a TXT record at {verification_name} with the value {expected}"
        )));
    }

    let target = normalize(&custom.target);
    let points = if target.parse::<std::net::IpAddr>().is_ok() {
        let a = lookup(state, &custom.domain, "A").await?;
        let aaaa = lookup(state, &custom.domain, "AAAA").await?;
        a.iter().chain(aaaa.iter()).any(|answer| {
            (answer.kind == 1 || answer.kind == 28) && normalize(&answer.data) == target
        })
    } else {
        let cname = lookup(state, &custom.domain, "CNAME").await?;
        cname
            .iter()
            .any(|answer| answer.kind == 5 && normalize(&answer.data) == target)
    };
    if !points {
        return Err(CustomDomainError::NotVerified(format!(
            "point the domain at {} with a CNAME record",
            custom.target
        )));
    }

    sqlx::query("UPDATE server_custom_domains SET verified_at = NOW() WHERE uuid = $1")
        .bind(custom.uuid)
        .execute(state.database.write())
        .await?;

    by_uuid(&state.database, custom.server_uuid, custom.uuid)
        .await?
        .ok_or(CustomDomainError::NotFound)
}

/// Removes a custom domain; the person's own DNS records are theirs to keep or remove.
pub async fn delete(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    uuid: uuid::Uuid,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM server_custom_domains WHERE server_uuid = $1 AND uuid = $2")
            .bind(server_uuid)
            .bind(uuid)
            .execute(state.database.write())
            .await?
            .rows_affected()
            > 0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_compared_without_case_or_trailing_dots() {
        assert_eq!(normalize("Play.Example.COM."), "play.example.com");
    }
}
