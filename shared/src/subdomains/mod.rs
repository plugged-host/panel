//! plugged.host: server subdomains (docs/features/subdomains.md): `play.example.com` for a
//! server, as DNS records on Cloudflare under the domains of Admin › Domains.
//!
//! Every subdomain gets an address record (A or AAAA) to the server's address; a game with an SRV
//! service (Minecraft, FiveM / RedM, the eggs' `srv-*` features) also gets its SRV records, so
//! players join without the port. The records' ids are kept in `server_subdomains.records`, and
//! every record carries a comment naming the panel and the server: only those are ever deleted.
//!
//! While tokens are on, a server's n-th subdomain costs the owner tokens by the subdomains
//! settings' tiers, refunded when it is deleted (with the server too). When the server moves to
//! another node (a transfer, a failover, a limbo restore) or changes its primary port, [`follow`]
//! points its records at the new address. The Discord settings can ask the server's owner to link
//! Discord or to be in the Discord server first ([`crate::discord::subdomain_gate`]).

pub mod cloudflare;
pub mod records;

use crate::{
    economy::{self, Currency},
    models::{ByUuid, server::Server},
};
use cloudflare::Cloudflare;
use records::Service;
use serde::Serialize;
use sqlx::Row;
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// Ledger types (shared/src/economy.rs).
pub const TYPE_SUBDOMAIN: &str = "subdomain";
pub const TYPE_REFUND: &str = "subdomain_refund";

/// How long new records are shown as still spreading.
const PROPAGATION_MINUTES: i64 = 3;

#[derive(Debug)]
pub enum SubdomainError {
    /// Subdomains are off, or the Cloudflare token is missing.
    Off,
    /// The server's egg runs nothing subdomains are made for.
    Unsupported,
    /// The server has no address (or port) a record could point at.
    NoAddress,
    /// The domain does not exist or is not linked to a Cloudflare zone.
    NoDomain,
    InvalidName,
    Blocked,
    Taken,
    Limit(i32),
    Tokens {
        cost: i32,
        balance: i64,
    },
    /// A record of that name exists on Cloudflare that is not this subdomain's.
    InUse(String),
    /// The Discord settings want the server's owner to link Discord, or to be in the server
    /// (docs/features/discord.md).
    DiscordLink,
    DiscordMember,
    Cloudflare(String),
    Database(anyhow::Error),
}

impl From<sqlx::Error> for SubdomainError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err.into())
    }
}

impl From<anyhow::Error> for SubdomainError {
    fn from(err: anyhow::Error) -> Self {
        Self::Database(err)
    }
}

impl From<cloudflare::CloudflareError> for SubdomainError {
    fn from(err: cloudflare::CloudflareError) -> Self {
        Self::Cloudflare(err.message)
    }
}

impl SubdomainError {
    pub fn message(&self) -> String {
        match self {
            Self::Off => "subdomains are not set up on this panel".into(),
            Self::Unsupported => "subdomains are not offered for this server's game".into(),
            Self::NoAddress => "the server has no address a subdomain could point at".into(),
            Self::NoDomain => "this domain cannot be used".into(),
            Self::InvalidName => "a subdomain is 2 to 63 lowercase letters, digits and hyphens, not starting or ending with a hyphen".into(),
            Self::Blocked => "this name is reserved".into(),
            Self::Taken => "this subdomain is taken".into(),
            Self::Limit(limit) => format!("this server may have {limit} subdomains"),
            Self::Tokens { cost, balance } => {
                format!("this subdomain costs {cost} tokens, the owner has {balance}")
            }
            Self::InUse(name) => format!("{name} is already in use"),
            Self::DiscordLink => {
                "the server's owner must link their Discord account for subdomains".into()
            }
            Self::DiscordMember => {
                "the server's owner must join the Discord server for subdomains".into()
            }
            Self::Cloudflare(message) => format!("Cloudflare: {message}"),
            Self::Database(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Taken | Self::Limit(_) | Self::InUse(_) => 409,
            Self::DiscordLink | Self::DiscordMember => 403,
            Self::Cloudflare(_) => 502,
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

/// A domain of Admin › Domains.
#[derive(ToSchema, Serialize, Clone)]
pub struct ApiDomain {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    /// Linked to its Cloudflare zone; until then nobody can use it.
    pub synced: bool,
    pub subdomains: i64,
    pub created: chrono::DateTime<chrono::Utc>,
}

/// A domain players can pick.
#[derive(ToSchema, Serialize, Clone)]
pub struct ApiDomainChoice {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
}

#[derive(Clone, Debug)]
pub struct Subdomain {
    pub uuid: uuid::Uuid,
    pub server_uuid: uuid::Uuid,
    pub owner_uuid: uuid::Uuid,
    pub domain_uuid: uuid::Uuid,
    pub domain_name: compact_str::CompactString,
    pub zone_id: Option<compact_str::CompactString>,
    pub name: compact_str::CompactString,
    pub record_type: compact_str::CompactString,
    pub records: BTreeMap<String, String>,
    pub token_cost: i32,
    pub created: chrono::NaiveDateTime,
}

#[derive(ToSchema, Serialize)]
pub struct ApiSubdomain {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub domain: compact_str::CompactString,
    pub fqdn: String,
    /// `A`, `AAAA` or `SRV` (with its address record).
    pub record_type: compact_str::CompactString,
    /// What players type to join.
    pub connect: String,
    /// Tokens paid, refunded on deletion.
    pub token_cost: i32,
    /// Made in the last few minutes: resolvers may not see it yet.
    pub propagating: bool,
    pub created: chrono::DateTime<chrono::Utc>,
}

const SELECT: &str = "SELECT server_subdomains.uuid, server_subdomains.server_uuid,
        servers.owner_uuid, server_subdomains.domain_uuid, domains.name AS domain_name,
        domains.zone_id, server_subdomains.name, server_subdomains.record_type,
        server_subdomains.records, server_subdomains.token_cost, server_subdomains.created
    FROM server_subdomains
    JOIN domains ON domains.uuid = server_subdomains.domain_uuid
    JOIN servers ON servers.uuid = server_subdomains.server_uuid";

impl Subdomain {
    fn map(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let records: serde_json::Value = row.try_get("records")?;

        Ok(Self {
            uuid: row.try_get("uuid")?,
            server_uuid: row.try_get("server_uuid")?,
            owner_uuid: row.try_get("owner_uuid")?,
            domain_uuid: row.try_get("domain_uuid")?,
            domain_name: row.try_get("domain_name")?,
            zone_id: row.try_get("zone_id")?,
            name: row.try_get("name")?,
            record_type: row.try_get("record_type")?,
            records: serde_json::from_value(records).unwrap_or_default(),
            token_cost: row.try_get("token_cost")?,
            created: row.try_get("created")?,
        })
    }

    pub fn fqdn(&self) -> String {
        format!("{}.{}", self.name, self.domain_name)
    }

    pub async fn all_by_server_uuid(
        database: &crate::database::Database,
        server_uuid: uuid::Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE server_subdomains.server_uuid = $1 ORDER BY server_subdomains.created"
        )))
        .bind(server_uuid)
        .fetch_all(database.read())
        .await?;

        rows.iter().map(Self::map).collect()
    }

    pub async fn by_server_uuid_uuid(
        database: &crate::database::Database,
        server_uuid: uuid::Uuid,
        uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE server_subdomains.server_uuid = $1 AND server_subdomains.uuid = $2"
        )))
        .bind(server_uuid)
        .bind(uuid)
        .fetch_optional(database.read())
        .await?;

        row.as_ref().map(Self::map).transpose()
    }

    async fn all_by_domain_uuid(
        database: &crate::database::Database,
        domain_uuid: uuid::Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE server_subdomains.domain_uuid = $1"
        )))
        .bind(domain_uuid)
        .fetch_all(database.write())
        .await?;

        rows.iter().map(Self::map).collect()
    }

    pub fn into_api(self, service: Option<Service>, port: Option<i32>) -> ApiSubdomain {
        let fqdn = self.fqdn();
        let created = self.created.and_utc();

        ApiSubdomain {
            uuid: self.uuid,
            connect: service.unwrap_or(Service::Plain).connect(&fqdn, port),
            fqdn,
            name: self.name,
            domain: self.domain_name,
            record_type: self.record_type,
            token_cost: self.token_cost,
            propagating: chrono::Utc::now() - created
                < chrono::Duration::minutes(PROPAGATION_MINUTES),
            created,
        }
    }
}

/// Where a server's records point.
#[derive(Clone, Debug)]
pub struct Target {
    pub ip: std::net::IpAddr,
    /// The host the SRV records name; `None` names the subdomain itself.
    pub srv_target: Option<String>,
    pub port: Option<i32>,
}

/// The host of a node URL, without the brackets of an IPv6 address.
pub(crate) fn host_of(url: &reqwest::Url) -> Option<String> {
    url.host_str().map(|host| {
        host.trim_start_matches('[')
            .trim_end_matches(']')
            .to_string()
    })
}

pub(crate) fn routable(ip: std::net::IpAddr) -> bool {
    !ip.is_unspecified() && !ip.is_loopback()
}

/// The address a host name (or IP) points at, IPv4 first; `None` for 0.0.0.0, loopback or a
/// lookup that fails or takes over 5 seconds.
pub(crate) async fn resolve(host: &str) -> Option<std::net::IpAddr> {
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return routable(ip).then_some(ip);
    }

    let addresses = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::lookup_host((host, 0)),
    )
    .await
    .ok()?
    .ok()?;
    let addresses: Vec<std::net::IpAddr> = addresses
        .map(|address| address.ip())
        .filter(|ip| routable(*ip))
        .collect();

    addresses
        .iter()
        .find(|ip| ip.is_ipv4())
        .or(addresses.first())
        .copied()
}

/// The address a server's records point at: its primary allocation's IP, or, for one that
/// listens on every address, what its alias, the node's SRV target or the node's address
/// resolves to. The SRV records name the node's SRV target or address when that is a host name.
pub async fn target(
    state: &crate::State,
    server: &Server,
) -> Result<Option<Target>, anyhow::Error> {
    let node = server.node.fetch_cached(&state.database).await?;
    let node_host = node
        .public_url
        .as_ref()
        .and_then(host_of)
        .or_else(|| host_of(&node.url));
    let srv_target = node
        .srv_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(str::to_string)
        .or_else(|| node_host.clone())
        .filter(|host| host.parse::<std::net::IpAddr>().is_err());

    let Some(allocation) = &server.allocation else {
        return Ok(None);
    };
    let port = Some(allocation.allocation.port);
    let ip = allocation.allocation.ip.ip();
    if routable(ip) {
        return Ok(Some(Target {
            ip,
            srv_target,
            port,
        }));
    }

    let candidates = [
        allocation
            .allocation
            .ip_alias
            .as_deref()
            .map(str::to_string),
        node.srv_target.as_deref().map(str::to_string),
        node_host,
    ];
    for candidate in candidates.into_iter().flatten() {
        let candidate = candidate.trim();
        if candidate.is_empty() {
            continue;
        }
        if let Some(ip) = resolve(candidate).await {
            return Ok(Some(Target {
                ip,
                srv_target,
                port,
            }));
        }
    }

    Ok(None)
}

/// What the subdomains settings let this server do right now.
struct Context {
    token: String,
    app_name: String,
    tokens_enabled: bool,
    settings: crate::settings::subdomains::AppSettingsSubdomains,
}

async fn context(state: &crate::State) -> Result<Context, SubdomainError> {
    let settings = state.settings.get().await?;
    if !settings.subdomains.active() {
        return Err(SubdomainError::Off);
    }

    Ok(Context {
        token: settings
            .subdomains
            .cloudflare_token
            .as_deref()
            .unwrap_or_default()
            .to_string(),
        app_name: settings.app.name.to_string(),
        tokens_enabled: settings.economy.tokens_enabled,
        settings: settings.subdomains.clone(),
    })
}

/// The cleanup helper's view of the settings: deleting records needs the token, not
/// subdomains being on.
async fn token_and_name(state: &crate::State) -> Result<Option<(String, String)>, anyhow::Error> {
    let settings = state.settings.get().await?;

    Ok(settings
        .subdomains
        .cloudflare_token
        .as_deref()
        .map(|token| (token.to_string(), settings.app.name.to_string())))
}

/// The subdomains the server may have: its own limit or the settings' default.
pub async fn limit(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
    default_limit: i32,
) -> Result<i32, sqlx::Error> {
    let own: Option<i32> =
        sqlx::query_scalar("SELECT subdomain_limit FROM servers WHERE uuid = $1")
            .bind(server_uuid)
            .fetch_optional(database.read())
            .await?
            .flatten();

    Ok(own.unwrap_or(default_limit))
}

/// The server's own limit, `None` when it follows the settings.
pub async fn own_limit(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<Option<i32>, sqlx::Error> {
    Ok(
        sqlx::query_scalar("SELECT subdomain_limit FROM servers WHERE uuid = $1")
            .bind(server_uuid)
            .fetch_optional(database.read())
            .await?
            .flatten(),
    )
}

pub async fn set_limit(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
    limit: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE servers SET subdomain_limit = $2 WHERE uuid = $1")
        .bind(server_uuid)
        .bind(limit)
        .execute(database.write())
        .await?;

    Ok(())
}

async fn synced_domains(
    database: &crate::database::Database,
) -> Result<
    Vec<(
        uuid::Uuid,
        compact_str::CompactString,
        compact_str::CompactString,
    )>,
    sqlx::Error,
> {
    let rows = sqlx::query(
        "SELECT uuid, name, zone_id FROM domains WHERE zone_id IS NOT NULL ORDER BY name",
    )
    .fetch_all(database.read())
    .await?;

    rows.iter()
        .map(|row| {
            Ok((
                row.try_get("uuid")?,
                row.try_get("name")?,
                row.try_get("zone_id")?,
            ))
        })
        .collect()
}

#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    /// Subdomains are off on this panel.
    Off,
    /// The server's game gets none.
    Unsupported,
    /// No domain is linked to Cloudflare yet.
    NoDomains,
    /// The server has no address to point at.
    NoAddress,
    /// The server's owner has not linked Discord (the Discord settings ask for it).
    DiscordLink,
    /// The server's owner is not in the Discord server (the Discord settings ask for it).
    DiscordMember,
}

/// A server's Subdomains page.
#[derive(ToSchema, Serialize)]
pub struct ApiOverview {
    pub subdomains: Vec<ApiSubdomain>,
    pub limit: i32,
    pub domains: Vec<ApiDomainChoice>,
    /// The game's records (`minecraft`, `fivem`, `teamspeak`, ..., `plain`); `None` when the
    /// game gets none.
    pub service: Option<&'static str>,
    /// What the address records point at.
    pub address: Option<String>,
    /// Why no subdomain can be added now.
    pub unavailable: Option<Unavailable>,
    /// Tokens the next subdomain costs (0 while tokens are off).
    pub next_cost: i32,
    /// The owner's tokens, shown to the owner only.
    pub balance: Option<i64>,
}

pub async fn overview(
    state: &crate::State,
    server: &Server,
    viewer_uuid: uuid::Uuid,
) -> Result<ApiOverview, anyhow::Error> {
    let settings = state.settings.get().await?;
    let active = settings.subdomains.active();
    let subdomains_settings = settings.subdomains.clone();
    let tokens_enabled = settings.economy.tokens_enabled;
    drop(settings);

    let service = Service::of_egg(&server.egg);
    let target = target(state, server).await?;
    let port = target.as_ref().and_then(|target| target.port);
    let subdomains = Subdomain::all_by_server_uuid(&state.database, server.uuid).await?;
    let domains = synced_domains(&state.database).await?;
    let limit = limit(
        &state.database,
        server.uuid,
        subdomains_settings.default_limit,
    )
    .await?;

    let unavailable = if !active {
        Some(Unavailable::Off)
    } else if service.is_none() {
        Some(Unavailable::Unsupported)
    } else if domains.is_empty() {
        Some(Unavailable::NoDomains)
    } else if target.is_none() {
        Some(Unavailable::NoAddress)
    } else {
        match crate::discord::subdomain_gate(state, server.owner.uuid, false).await? {
            crate::discord::Gate::Open => None,
            crate::discord::Gate::NeedsLink => Some(Unavailable::DiscordLink),
            crate::discord::Gate::NeedsMember => Some(Unavailable::DiscordMember),
        }
    };

    let balance = if tokens_enabled && server.owner.uuid == viewer_uuid {
        sqlx::query_scalar::<_, i64>("SELECT tokens FROM users WHERE uuid = $1")
            .bind(server.owner.uuid)
            .fetch_optional(state.database.read())
            .await?
    } else {
        None
    };

    Ok(ApiOverview {
        next_cost: if tokens_enabled {
            subdomains_settings.cost(subdomains.len() as i64)
        } else {
            0
        },
        subdomains: subdomains
            .into_iter()
            .map(|subdomain| subdomain.into_api(service, port))
            .collect(),
        limit,
        domains: domains
            .into_iter()
            .map(|(uuid, name, _)| ApiDomainChoice { uuid, name })
            .collect(),
        service: service.map(Service::key),
        address: target.map(|target| target.ip.to_string()),
        unavailable,
        balance,
    })
}

/// Writes the subdomain's records for the server's game and address and stores their ids. A
/// failure leaves the records it had before (new ones it made are removed again).
async fn write_records(
    state: &crate::State,
    cloudflare: &Cloudflare<'_>,
    app_name: &str,
    subdomain: &mut Subdomain,
    service: Service,
    target: &Target,
) -> Result<(), SubdomainError> {
    let zone = subdomain.zone_id.clone().ok_or(SubdomainError::NoDomain)?;
    let fqdn = subdomain.fqdn();
    let detail = records::server_detail(subdomain.server_uuid);
    let ip_type = if target.ip.is_ipv6() { "AAAA" } else { "A" };

    let mut wanted: Vec<(&'static str, String, &'static str, serde_json::Value)> = vec![(
        "a",
        fqdn.clone(),
        ip_type,
        serde_json::json!({
            "type": ip_type,
            "name": fqdn,
            "content": target.ip.to_string(),
            "ttl": 1,
            "proxied": false,
            "comment": records::comment(app_name, &detail),
        }),
    )];
    if !service.srv().is_empty() {
        let port = target.port.ok_or(SubdomainError::NoAddress)?;
        let srv_target = target.srv_target.clone().unwrap_or_else(|| fqdn.clone());

        for spec in service.srv() {
            let name = format!("{}.{fqdn}", spec.prefix);
            wanted.push((
                spec.key,
                name.clone(),
                "SRV",
                serde_json::json!({
                    "type": "SRV",
                    "name": name,
                    "ttl": 1,
                    "comment": records::comment(
                        app_name,
                        &format!("{} SRV - {detail}", spec.prefix),
                    ),
                    "data": {
                        "priority": 0,
                        "weight": spec.weight,
                        "port": port,
                        "target": srv_target,
                    },
                }),
            ));
        }
    }

    // nothing of these names and types may exist but this subdomain's own records
    for (key, name, kind, _) in &wanted {
        let own = subdomain.records.get(*key);
        if cloudflare
            .records(&zone, name, Some(kind))
            .await?
            .iter()
            .any(|record| Some(&record.id) != own)
        {
            return Err(SubdomainError::InUse(name.clone()));
        }
    }

    let mut written = BTreeMap::new();
    let mut created = Vec::new();
    for (key, _, _, body) in &wanted {
        let result = match subdomain.records.get(*key) {
            Some(id) => match cloudflare.update(&zone, id, body).await {
                Err(err) if err.missing() => {
                    cloudflare.create(&zone, body).await.inspect(|record| {
                        created.push(record.id.clone());
                    })
                }
                result => result,
            },
            None => cloudflare.create(&zone, body).await.inspect(|record| {
                created.push(record.id.clone());
            }),
        };

        match result {
            Ok(record) => {
                written.insert(key.to_string(), record.id);
            }
            Err(err) => {
                for id in &created {
                    cloudflare.delete(&zone, id).await.ok();
                }

                return Err(err.into());
            }
        }
    }

    // records of a game the server no longer runs
    for (key, id) in &subdomain.records {
        if !written.contains_key(key)
            && let Err(err) = cloudflare.delete(&zone, id).await
        {
            tracing::warn!(subdomain = %subdomain.uuid, "failed to delete an old record: {err}");
        }
    }

    let record_type = if service.srv().is_empty() {
        ip_type
    } else {
        "SRV"
    };
    sqlx::query("UPDATE server_subdomains SET records = $2, record_type = $3 WHERE uuid = $1")
        .bind(subdomain.uuid)
        .bind(serde_json::to_value(&written).unwrap_or_default())
        .bind(record_type)
        .execute(state.database.write())
        .await?;
    subdomain.records = written;
    subdomain.record_type = record_type.into();

    Ok(())
}

/// Deletes the subdomain's records on Cloudflare: the stored ids, and any other record of its
/// names whose comment says the panel made it for this server.
async fn delete_records(
    cloudflare: &Cloudflare<'_>,
    app_name: &str,
    subdomain: &Subdomain,
    service: Option<Service>,
) {
    let Some(zone) = subdomain.zone_id.as_deref() else {
        return;
    };

    for id in subdomain.records.values() {
        if let Err(err) = cloudflare.delete(zone, id).await {
            tracing::warn!(subdomain = %subdomain.uuid, "failed to delete a record: {err}");
        }
    }

    let fqdn = subdomain.fqdn();
    let mut names = vec![fqdn.clone()];
    names.extend(
        service
            .unwrap_or(Service::Plain)
            .srv()
            .iter()
            .map(|spec| format!("{}.{fqdn}", spec.prefix)),
    );
    for name in names {
        let Ok(found) = cloudflare.records(zone, &name, None).await else {
            continue;
        };
        for record in found {
            if !subdomain.records.values().any(|id| id == &record.id)
                && records::own_comment(record.comment.as_deref(), app_name, subdomain.server_uuid)
            {
                cloudflare.delete(zone, &record.id).await.ok();
            }
        }
    }
}

/// Removes the row and refunds its tokens, in one transaction. `None` when another request
/// removed it first (and refunded it).
async fn remove_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subdomain: &Subdomain,
) -> Result<Option<i32>, sqlx::Error> {
    let cost: Option<i32> =
        sqlx::query_scalar("DELETE FROM server_subdomains WHERE uuid = $1 RETURNING token_cost")
            .bind(subdomain.uuid)
            .fetch_optional(&mut **transaction)
            .await?;
    let Some(cost) = cost else {
        return Ok(None);
    };

    if cost > 0 {
        economy::change(
            transaction,
            Currency::Tokens,
            subdomain.owner_uuid,
            cost as i64,
            TYPE_REFUND,
            &format!("Subdomain {} deleted", subdomain.fqdn()),
            serde_json::json!({ "server_uuid": subdomain.server_uuid, "subdomain": subdomain.fqdn() }),
        )
        .await?;
    }

    Ok(Some(cost))
}

pub struct Create {
    pub name: String,
    pub domain_uuid: Option<uuid::Uuid>,
}

/// Adds a subdomain: the owner pays its tier, the row is written, then the records. When the
/// records cannot be made, the row goes again and the tokens come back.
pub async fn create(
    state: &crate::State,
    server: &Server,
    request: Create,
) -> Result<Subdomain, SubdomainError> {
    let context = context(state).await?;
    let service = Service::of_egg(&server.egg).ok_or(SubdomainError::Unsupported)?;
    match crate::discord::subdomain_gate(state, server.owner.uuid, true).await? {
        crate::discord::Gate::Open => {}
        crate::discord::Gate::NeedsLink => return Err(SubdomainError::DiscordLink),
        crate::discord::Gate::NeedsMember => return Err(SubdomainError::DiscordMember),
    }

    let name = request.name.trim().to_lowercase();
    if !records::valid_name(&name) {
        return Err(SubdomainError::InvalidName);
    }
    if records::blocked(&name, &context.settings.blocked_names) {
        return Err(SubdomainError::Blocked);
    }

    let domains = synced_domains(&state.database).await?;
    let (domain_uuid, domain_name, zone_id) = match request.domain_uuid {
        Some(uuid) => domains.into_iter().find(|(domain, _, _)| *domain == uuid),
        None => domains.into_iter().next(),
    }
    .ok_or(SubdomainError::NoDomain)?;

    let target = target(state, server)
        .await?
        .ok_or(SubdomainError::NoAddress)?;

    let mut transaction = state.database.write().begin().await?;
    let own_limit: Option<i32> =
        sqlx::query_scalar("SELECT subdomain_limit FROM servers WHERE uuid = $1 FOR UPDATE")
            .bind(server.uuid)
            .fetch_one(&mut *transaction)
            .await?;
    let limit = own_limit.unwrap_or(context.settings.default_limit);
    let existing: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM server_subdomains WHERE server_uuid = $1")
            .bind(server.uuid)
            .fetch_one(&mut *transaction)
            .await?;
    if existing >= limit as i64 {
        return Err(SubdomainError::Limit(limit));
    }

    let fqdn = format!("{name}.{domain_name}");
    let cost = if context.tokens_enabled {
        context.settings.cost(existing).max(0)
    } else {
        0
    };
    if cost > 0
        && economy::change(
            &mut transaction,
            Currency::Tokens,
            server.owner.uuid,
            -(cost as i64),
            TYPE_SUBDOMAIN,
            &format!("Subdomain {fqdn}"),
            serde_json::json!({ "server_uuid": server.uuid, "subdomain": fqdn }),
        )
        .await?
        .is_none()
    {
        let balance: i64 = sqlx::query_scalar("SELECT tokens FROM users WHERE uuid = $1")
            .bind(server.owner.uuid)
            .fetch_one(&mut *transaction)
            .await?;

        return Err(SubdomainError::Tokens { cost, balance });
    }

    let inserted = sqlx::query(
        "INSERT INTO server_subdomains (server_uuid, domain_uuid, name, record_type, token_cost)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING uuid, created",
    )
    .bind(server.uuid)
    .bind(domain_uuid)
    .bind(&name)
    .bind(if service.srv().is_empty() { "A" } else { "SRV" })
    .bind(cost)
    .fetch_one(&mut *transaction)
    .await;
    let row = match inserted {
        Ok(row) => row,
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            return Err(SubdomainError::Taken);
        }
        Err(err) => return Err(err.into()),
    };
    transaction.commit().await?;

    let mut subdomain = Subdomain {
        uuid: row.try_get("uuid")?,
        server_uuid: server.uuid,
        owner_uuid: server.owner.uuid,
        domain_uuid,
        domain_name,
        zone_id: Some(zone_id),
        name: name.into(),
        record_type: compact_str::CompactString::default(),
        records: BTreeMap::new(),
        token_cost: cost,
        created: row.try_get("created")?,
    };

    let cloudflare = Cloudflare::new(&state.client, &context.token);
    if let Err(err) = write_records(
        state,
        &cloudflare,
        &context.app_name,
        &mut subdomain,
        service,
        &target,
    )
    .await
    {
        let mut transaction = state.database.write().begin().await?;
        remove_row(&mut transaction, &subdomain).await?;
        transaction.commit().await?;

        return Err(err);
    }

    Ok(subdomain)
}

/// Renames a subdomain, or with the same name writes its records again (after a change Cloudflare
/// did not get). On failure the old name is kept and its records written back.
pub async fn update(
    state: &crate::State,
    server: &Server,
    mut subdomain: Subdomain,
    name: Option<String>,
) -> Result<Subdomain, SubdomainError> {
    let context = context(state).await?;
    let service = Service::of_egg(&server.egg).ok_or(SubdomainError::Unsupported)?;
    let target = target(state, server)
        .await?
        .ok_or(SubdomainError::NoAddress)?;
    let cloudflare = Cloudflare::new(&state.client, &context.token);

    let old_name = subdomain.name.clone();
    if let Some(name) = name.map(|name| name.trim().to_lowercase())
        && name != old_name
    {
        if !records::valid_name(&name) {
            return Err(SubdomainError::InvalidName);
        }
        if records::blocked(&name, &context.settings.blocked_names) {
            return Err(SubdomainError::Blocked);
        }

        match sqlx::query("UPDATE server_subdomains SET name = $2 WHERE uuid = $1")
            .bind(subdomain.uuid)
            .bind(&name)
            .execute(state.database.write())
            .await
        {
            Ok(_) => {}
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
                return Err(SubdomainError::Taken);
            }
            Err(err) => return Err(err.into()),
        }
        subdomain.name = name.into();
    }

    match write_records(
        state,
        &cloudflare,
        &context.app_name,
        &mut subdomain,
        service,
        &target,
    )
    .await
    {
        Ok(()) => Ok(subdomain),
        Err(err) if subdomain.name != old_name => {
            sqlx::query("UPDATE server_subdomains SET name = $2 WHERE uuid = $1")
                .bind(subdomain.uuid)
                .bind(&*old_name)
                .execute(state.database.write())
                .await?;
            subdomain.name = old_name;
            write_records(
                state,
                &cloudflare,
                &context.app_name,
                &mut subdomain,
                service,
                &target,
            )
            .await
            .ok();

            Err(err)
        }
        Err(err) => Err(err),
    }
}

/// Deletes a subdomain: the row and the refund first, then its records. `None` when it was gone
/// already.
pub async fn delete(
    state: &crate::State,
    subdomain: &Subdomain,
    service: Option<Service>,
) -> Result<Option<i32>, anyhow::Error> {
    let mut transaction = state.database.write().begin().await?;
    let refunded = remove_row(&mut transaction, subdomain).await?;
    transaction.commit().await?;

    if refunded.is_some()
        && let Some((token, app_name)) = token_and_name(state).await?
    {
        delete_records(
            &Cloudflare::new(&state.client, &token),
            &app_name,
            subdomain,
            service,
        )
        .await;
    }

    Ok(refunded)
}

/// Points a server's records at where it is now, in the background: after a transfer, a
/// failover or limbo restore ([`Server::rehome`]) or a new primary allocation.
pub fn follow(state: &crate::State, server_uuid: uuid::Uuid) {
    let state = state.clone();

    tokio::spawn(async move {
        if let Err(err) = follow_now(&state, server_uuid).await {
            tracing::warn!(server = %server_uuid, "failed to update subdomains: {err:#}");
        }
    });
}

async fn follow_now(state: &crate::State, server_uuid: uuid::Uuid) -> Result<(), anyhow::Error> {
    let subdomains = Subdomain::all_by_server_uuid(&state.database, server_uuid).await?;
    if subdomains.is_empty() {
        return Ok(());
    }
    let Some((token, app_name)) = token_and_name(state).await? else {
        return Ok(());
    };
    let Some(server) = Server::by_uuid_optional(&state.database, server_uuid).await? else {
        return Ok(());
    };
    let Some(service) = Service::of_egg(&server.egg) else {
        return Ok(());
    };
    let Some(target) = target(state, &server).await? else {
        tracing::warn!(server = %server_uuid, "no address to point subdomains at");
        return Ok(());
    };

    let cloudflare = Cloudflare::new(&state.client, &token);
    for mut subdomain in subdomains {
        if let Err(err) = write_records(
            state,
            &cloudflare,
            &app_name,
            &mut subdomain,
            service,
            &target,
        )
        .await
        {
            tracing::warn!(
                server = %server_uuid,
                subdomain = %subdomain.fqdn(),
                "failed to update a subdomain: {}",
                err.message()
            );
        }
    }

    Ok(())
}

/// For a server being deleted, in its delete transaction: its subdomains' rows go and their
/// tokens come back. Hand the result to [`forget`] once the transaction is committed.
pub async fn release(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    server_uuid: uuid::Uuid,
) -> Result<Vec<Subdomain>, sqlx::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE server_subdomains.server_uuid = $1 FOR UPDATE OF server_subdomains"
    )))
    .bind(server_uuid)
    .fetch_all(&mut **transaction)
    .await?;
    let subdomains = rows
        .iter()
        .map(Subdomain::map)
        .collect::<Result<Vec<_>, _>>()?;

    let mut released = Vec::with_capacity(subdomains.len());
    for subdomain in subdomains {
        if remove_row(transaction, &subdomain).await?.is_some() {
            released.push(subdomain);
        }
    }

    Ok(released)
}

/// Deletes the records of subdomains [`release`]d, in the background.
pub fn forget(state: &crate::State, subdomains: Vec<Subdomain>, service: Option<Service>) {
    if subdomains.is_empty() {
        return;
    }
    let state = state.clone();

    tokio::spawn(async move {
        let Ok(Some((token, app_name))) = token_and_name(&state).await else {
            return;
        };
        let cloudflare = Cloudflare::new(&state.client, &token);
        for subdomain in &subdomains {
            delete_records(&cloudflare, &app_name, subdomain, service).await;
        }
    });
}

#[derive(Debug)]
pub enum DomainError {
    /// No Cloudflare token is set.
    NoToken,
    Exists,
    InvalidName,
    /// The token sees no zone of that name.
    NoZone,
    Cloudflare(String),
    Database(anyhow::Error),
}

impl From<sqlx::Error> for DomainError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err.into())
    }
}

impl From<anyhow::Error> for DomainError {
    fn from(err: anyhow::Error) -> Self {
        Self::Database(err)
    }
}

impl DomainError {
    pub fn message(&self) -> String {
        match self {
            Self::NoToken => {
                "set the Cloudflare token in Admin › Settings › Hosting › Subdomains first".into()
            }
            Self::Exists => "this domain is added already".into(),
            Self::InvalidName => "this is not a domain name".into(),
            Self::NoZone => "the Cloudflare token sees no zone of this name".into(),
            Self::Cloudflare(message) => format!("Cloudflare: {message}"),
            Self::Database(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Exists => 409,
            Self::Cloudflare(_) => 502,
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

/// Whether `name` is a domain name (`example.com`, `games.example.org`).
pub fn valid_domain(name: &str) -> bool {
    name.len() <= 253
        && name.contains('.')
        && name.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

const DOMAIN_SELECT: &str = "SELECT domains.uuid, domains.name, domains.zone_id, domains.created,
        (SELECT COUNT(*) FROM server_subdomains WHERE server_subdomains.domain_uuid = domains.uuid) AS subdomains
    FROM domains";

fn map_domain(row: &sqlx::postgres::PgRow) -> Result<ApiDomain, sqlx::Error> {
    let zone_id: Option<compact_str::CompactString> = row.try_get("zone_id")?;
    let created: chrono::NaiveDateTime = row.try_get("created")?;

    Ok(ApiDomain {
        uuid: row.try_get("uuid")?,
        name: row.try_get("name")?,
        synced: zone_id.is_some(),
        subdomains: row.try_get("subdomains")?,
        created: created.and_utc(),
    })
}

pub async fn domains(database: &crate::database::Database) -> Result<Vec<ApiDomain>, sqlx::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{DOMAIN_SELECT} ORDER BY domains.name"
    )))
    .fetch_all(database.read())
    .await?;

    rows.iter().map(map_domain).collect()
}

pub async fn domain(
    database: &crate::database::Database,
    uuid: uuid::Uuid,
) -> Result<Option<ApiDomain>, sqlx::Error> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "{DOMAIN_SELECT} WHERE domains.uuid = $1"
    )))
    .bind(uuid)
    .fetch_optional(database.write())
    .await?;

    row.as_ref().map(map_domain).transpose()
}

async fn cloudflare_token(state: &crate::State) -> Result<String, DomainError> {
    token_and_name(state)
        .await?
        .map(|(token, _)| token)
        .ok_or(DomainError::NoToken)
}

/// Adds a domain and links it to its Cloudflare zone. A domain whose zone the token does not see
/// is refused.
pub async fn add_domain(state: &crate::State, name: &str) -> Result<ApiDomain, DomainError> {
    let name = name.trim().trim_end_matches('.').to_lowercase();
    if !valid_domain(&name) {
        return Err(DomainError::InvalidName);
    }
    let token = cloudflare_token(state).await?;
    let zone = Cloudflare::new(&state.client, &token)
        .zone_id(&name)
        .await
        .map_err(|err| DomainError::Cloudflare(err.message))?
        .ok_or(DomainError::NoZone)?;

    let uuid = match sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO domains (name, zone_id) VALUES ($1, $2) RETURNING uuid",
    )
    .bind(&name)
    .bind(&zone)
    .fetch_one(state.database.write())
    .await
    {
        Ok(uuid) => uuid,
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            return Err(DomainError::Exists);
        }
        Err(err) => return Err(err.into()),
    };

    domain(&state.database, uuid)
        .await?
        .ok_or_else(|| DomainError::Database(anyhow::anyhow!("the domain went missing")))
}

/// Looks the domain's zone up again (after it moved to another Cloudflare account).
pub async fn sync_domain(state: &crate::State, uuid: uuid::Uuid) -> Result<ApiDomain, DomainError> {
    let Some(current) = domain(&state.database, uuid).await? else {
        return Err(DomainError::InvalidName);
    };
    let token = cloudflare_token(state).await?;
    let zone = Cloudflare::new(&state.client, &token)
        .zone_id(&current.name)
        .await
        .map_err(|err| DomainError::Cloudflare(err.message))?;

    sqlx::query("UPDATE domains SET zone_id = $2 WHERE uuid = $1")
        .bind(uuid)
        .bind(&zone)
        .execute(state.database.write())
        .await?;

    let synced = domain(&state.database, uuid)
        .await?
        .ok_or(DomainError::InvalidName)?;
    if zone.is_none() {
        return Err(DomainError::NoZone);
    }

    Ok(synced)
}

/// Deletes a domain with its subdomains (their records go and their tokens come back).
pub async fn delete_domain(state: &crate::State, uuid: uuid::Uuid) -> Result<u64, anyhow::Error> {
    for subdomain in Subdomain::all_by_domain_uuid(&state.database, uuid).await? {
        let service = match Server::by_uuid_optional(&state.database, subdomain.server_uuid).await?
        {
            Some(server) => Service::of_egg(&server.egg),
            None => None,
        };
        delete(state, &subdomain, service).await?;
    }

    Ok(sqlx::query("DELETE FROM domains WHERE uuid = $1")
        .bind(uuid)
        .execute(state.database.write())
        .await?
        .rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_names() {
        assert!(valid_domain("example.com"));
        assert!(valid_domain("games.example.co.uk"));
        assert!(!valid_domain("localhost"));
        assert!(!valid_domain("-bad.example.com"));
        assert!(!valid_domain("Example.com"));
        assert!(!valid_domain("a..b"));
    }

    #[test]
    fn errors_have_statuses() {
        assert_eq!(SubdomainError::Taken.status(), 409);
        assert_eq!(SubdomainError::Cloudflare("x".into()).status(), 502);
        assert_eq!(SubdomainError::Blocked.status(), 400);
        assert_eq!(DomainError::Exists.status(), 409);
    }
}
