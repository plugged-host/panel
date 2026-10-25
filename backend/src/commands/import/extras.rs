// plugged.host: what an import from the Pelican panel brings besides upstream's tables
// (docs/features/import.md): noprofit.space links, the economy (token and credit balances, their
// histories, memberships), Discord members, invites and boost payouts, domains and subdomains,
// server icons and the console font size, and it writes the location and node fields of
// mapping.rs. Each source table is optional, so a plain Pelican panel imports as before.
// Devboxes and the servers behind them are left out.

use super::{
    SourceColumns, SourceContext, SourceRow, model::*, source_bool, source_i64,
    source_optional_i64, source_optional_text, source_text, source_timestamp,
};
use anyhow::Context;
use chrono::{DateTime, NaiveDate, Utc};
use colored::Colorize;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

/// Where the Pelican panel keeps server icons, below its root (Laravel's public disk).
const ICON_DIRECTORY: &str = "storage/app/public/icons/server";
/// The icon formats the Pelican panel stores, in the order it looks for them.
const ICON_EXTENSIONS: [&str; 3] = ["png", "jpg", "webp"];
/// Calagopus' console font size when none is set (`console::font_size`).
const DEFAULT_CONSOLE_FONT_SIZE: i64 = 14;
const LEFT_OUT_USER: &str = "rows of users that were not imported were left out";

pub struct SourceUserExtras {
    pub id: i64,
    pub noprofit_account_id: Option<i64>,
    pub console_font_size: Option<i64>,
}

pub struct SourceServerExtras {
    pub id: i64,
    pub subdomain_limit: Option<i64>,
    pub icon: Option<PathBuf>,
}

pub struct SourceTokens {
    pub user_id: i64,
    pub tokens: i64,
    pub earned: i64,
    pub daily_granted: Option<NaiveDate>,
}

pub struct SourceCredits {
    pub user_id: i64,
    pub credits: i64,
    pub extra_cpu: i64,
    pub extra_memory: i64,
    pub extra_disk: i64,
}

pub struct SourceTransaction {
    pub user_id: i64,
    pub amount: i64,
    pub balance_after: i64,
    pub kind: String,
    pub description: String,
    pub metadata: Option<String>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceMembership {
    pub user_id: i64,
    pub plus_since: Option<DateTime<Utc>>,
    pub plusplus_until: Option<DateTime<Utc>>,
    pub plusplus_settled: Option<DateTime<Utc>>,
    pub plusplus_purchases: i64,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceDiscordLink {
    pub user_id: i64,
    pub discord_id: Option<String>,
    pub username: Option<String>,
    pub avatar: Option<String>,
    pub in_guild: bool,
    pub invite_debt: i64,
    pub boost_count: i64,
    pub boosts_rewarded: i64,
    pub boosting: bool,
    pub boosted_since: Option<DateTime<Utc>>,
    pub verified: Option<DateTime<Utc>>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceDiscordInvite {
    pub inviter: String,
    pub joined: String,
    pub invite_code: Option<String>,
    pub account_created: Option<DateTime<Utc>>,
    pub joined_at: Option<DateTime<Utc>>,
    pub eligible: bool,
    pub rewarded: bool,
    pub rewarded_at: Option<DateTime<Utc>>,
    pub matures: Option<DateTime<Utc>>,
    pub left: bool,
    pub left_at: Option<DateTime<Utc>>,
    pub leave_count: i64,
    pub rejoin_count: i64,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_credits: i64,
    pub last_seen: Option<DateTime<Utc>>,
    pub created: Option<DateTime<Utc>>,
}

/// The Pelican Discord plugin's rewards, from its environment (its defaults otherwise): what an
/// invite paid, the cumulative credits of boost levels 1 and 2, and the hours before a join counts.
pub struct DiscordRewards {
    pub per_invite: i64,
    pub per_boost: [i64; 2],
    pub maturity_hours: i64,
}

impl Default for DiscordRewards {
    fn default() -> Self {
        Self {
            per_invite: 50,
            per_boost: [500, 1250],
            maturity_hours: 6,
        }
    }
}

impl DiscordRewards {
    /// The credits paid for reaching `level`: the Pelican settings are totals per level.
    pub fn boost_credits(&self, level: i64) -> i64 {
        match level {
            1 => self.per_boost[0].max(0),
            2 => (self.per_boost[1] - self.per_boost[0]).max(0),
            _ => 0,
        }
    }
}

pub struct SourceDomain {
    pub id: i64,
    pub name: String,
    pub zone_id: Option<String>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceSubdomain {
    pub id: i64,
    pub name: String,
    pub record_type: Option<String>,
    pub token_cost: i64,
    pub records: Option<String>,
    pub domain_id: i64,
    pub server_id: i64,
    pub created: Option<DateTime<Utc>>,
}

/// The Pelican devbox plugin's rows: the hidden servers it runs and the mounts it gave them.
#[derive(Default)]
pub struct SourceDevboxes {
    pub server_ids: HashSet<i64>,
    pub mount_ids: HashSet<i64>,
}

#[derive(Default)]
pub struct SourceExtras {
    pub users: Vec<SourceUserExtras>,
    pub servers: Vec<SourceServerExtras>,
    pub tokens: Vec<SourceTokens>,
    pub token_transactions: Vec<SourceTransaction>,
    pub credits: Vec<SourceCredits>,
    pub credit_transactions: Vec<SourceTransaction>,
    pub memberships: Vec<SourceMembership>,
    pub discord_links: Vec<SourceDiscordLink>,
    pub discord_invites: Vec<SourceDiscordInvite>,
    pub discord: DiscordRewards,
    pub domains: Vec<SourceDomain>,
    pub subdomains: Vec<SourceSubdomain>,
    pub devboxes: Option<SourceDevboxes>,
    /// Notes from reading, reported with validation's.
    pub notes: Vec<(&'static str, String)>,
    /// Read from a Pelican panel: the permission rules of its plugins apply (mapping.rs).
    pub pelican: bool,
    /// The limbo plugin's parked and stopped servers (limbo.rs).
    pub limbo: super::limbo::SourceLimbo,
}

fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().trim_matches('"').to_string())
        .filter(|value| !value.is_empty() && value != "null")
}

fn env_number(key: &str, default: i64) -> i64 {
    env_value(key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn optional_i64(
    table: &SourceColumns,
    row: &SourceRow,
    column: &str,
) -> Result<Option<i64>, anyhow::Error> {
    if table.has(column) {
        source_optional_i64(row, column)
    } else {
        Ok(None)
    }
}

/// A yes/no column, no when it is missing or empty.
fn flag(table: &SourceColumns, row: &SourceRow, column: &str) -> Result<bool, anyhow::Error> {
    Ok(table.has(column) && source_bool(row, column).unwrap_or(false))
}

fn source_date(row: &SourceRow, column: &str) -> Option<NaiveDate> {
    let value = source_optional_text(row, column).ok()??;

    NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()
}

/// The console font size in a Pelican user's `customization` (JSON, sometimes JSON in a JSON
/// string), when it is a sensible size other than the default.
pub fn console_font_size(customization: Option<&str>) -> Option<i64> {
    let mut value: serde_json::Value = serde_json::from_str(customization?).ok()?;
    if let serde_json::Value::String(inner) = &value {
        value = serde_json::from_str(inner).ok()?;
    }

    let size = match value.get("console_font_size")? {
        serde_json::Value::Number(size) => size.as_i64()?,
        serde_json::Value::String(size) => size.trim().parse().ok()?,
        _ => return None,
    };

    ((6..=64).contains(&size) && size != DEFAULT_CONSOLE_FONT_SIZE).then_some(size)
}

/// The Pelican panel's icon file of a server, if it has one.
fn icon_file(directory: &Path, server_uuid: &str) -> Option<PathBuf> {
    let server_uuid: uuid::Uuid = server_uuid.trim().parse().ok()?;

    ICON_EXTENSIONS
        .iter()
        .map(|extension| directory.join(format!("{server_uuid}.{extension}")))
        .find(|path| path.is_file())
}

async fn read_transactions(
    context: &SourceContext,
    table_name: &str,
) -> Result<Vec<SourceTransaction>, anyhow::Error> {
    if !context.has_table(table_name).await? {
        return Ok(Vec::new());
    }

    context
        .map_table(table_name, None, |table, row| {
            Ok(SourceTransaction {
                user_id: source_i64(row, "user_id")?,
                amount: source_i64(row, "amount")?,
                balance_after: optional_i64(table, row, "balance_after")?.unwrap_or(0),
                kind: source_text(row, "type")?,
                description: source_optional_text(row, "description")?.unwrap_or_default(),
                metadata: source_optional_text(row, "metadata")?,
                created: source_timestamp(row, "created_at"),
            })
        })
        .await
}

/// Reads what the Pelican panel's plugins keep; a table or column that is not there reads as empty.
pub(super) async fn read(
    context: &SourceContext,
    environment: &str,
) -> Result<SourceExtras, anyhow::Error> {
    let mut extras = SourceExtras {
        pelican: true,
        ..Default::default()
    };

    extras.users = context
        .map_table("users", None, |table, row| {
            Ok(SourceUserExtras {
                id: source_i64(row, "id")?,
                noprofit_account_id: optional_i64(table, row, "noprofit_account_id")?,
                console_font_size: console_font_size(
                    source_optional_text(row, "customization")?.as_deref(),
                ),
            })
        })
        .await?;

    let icons = Path::new(environment)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(ICON_DIRECTORY);
    let icons = icons.is_dir().then_some(icons);
    extras.servers = context
        .map_table("servers", None, |table, row| {
            Ok(SourceServerExtras {
                id: source_i64(row, "id")?,
                subdomain_limit: optional_i64(table, row, "subdomain_limit")?,
                icon: icons.as_deref().and_then(|icons| {
                    icon_file(icons, &source_optional_text(row, "uuid").ok().flatten()?)
                }),
            })
        })
        .await?;

    if context.has_table("user_tokens").await? {
        extras.tokens = context
            .map_table("user_tokens", None, |table, row| {
                Ok(SourceTokens {
                    user_id: source_i64(row, "user_id")?,
                    tokens: source_i64(row, "tokens")?,
                    earned: optional_i64(table, row, "lifetime_earned")?.unwrap_or(0),
                    daily_granted: source_date(row, "daily_granted_on"),
                })
            })
            .await?;
    }
    extras.token_transactions = read_transactions(context, "token_transactions").await?;

    if context.has_table("user_credits").await? {
        extras.credits = context
            .map_table("user_credits", None, |table, row| {
                Ok(SourceCredits {
                    user_id: source_i64(row, "user_id")?,
                    credits: source_i64(row, "credits")?,
                    extra_cpu: optional_i64(table, row, "extra_cpu")?.unwrap_or(0),
                    extra_memory: optional_i64(table, row, "extra_memory")?.unwrap_or(0),
                    extra_disk: optional_i64(table, row, "extra_disk")?.unwrap_or(0),
                })
            })
            .await?;
    }
    extras.credit_transactions = read_transactions(context, "credit_transactions").await?;

    if context.has_table("user_memberships").await? {
        extras.memberships = context
            .map_table("user_memberships", None, |table, row| {
                Ok(SourceMembership {
                    user_id: source_i64(row, "user_id")?,
                    plus_since: source_timestamp(row, "plus_at"),
                    plusplus_until: source_timestamp(row, "plusplus_until"),
                    plusplus_settled: source_timestamp(row, "plusplus_settled_at"),
                    plusplus_purchases: optional_i64(table, row, "plusplus_purchases")?
                        .unwrap_or(0),
                    created: source_timestamp(row, "created_at"),
                })
            })
            .await?;
    }

    // the OAuth tokens in this table are never used
    if context.has_table("discord_links").await? {
        extras.discord_links = context
            .map_table("discord_links", None, |table, row| {
                Ok(SourceDiscordLink {
                    user_id: source_i64(row, "user_id")?,
                    discord_id: source_optional_text(row, "discord_id")?,
                    username: source_optional_text(row, "discord_username")?,
                    avatar: source_optional_text(row, "discord_avatar")?,
                    in_guild: flag(table, row, "is_in_guild")?,
                    invite_debt: optional_i64(table, row, "invite_debt")?.unwrap_or(0),
                    boost_count: optional_i64(table, row, "boost_count")?.unwrap_or(0),
                    boosts_rewarded: optional_i64(table, row, "boosts_rewarded")?.unwrap_or(0),
                    boosting: flag(table, row, "is_boosting")?,
                    boosted_since: source_timestamp(row, "boosted_since"),
                    verified: source_timestamp(row, "last_verified_at"),
                    created: source_timestamp(row, "created_at"),
                })
            })
            .await?;
    }

    if context.has_table("discord_invited_members").await? {
        extras.discord_invites = context
            .map_table("discord_invited_members", None, |table, row| {
                Ok(SourceDiscordInvite {
                    inviter: source_text(row, "inviter_discord_id")?,
                    joined: source_text(row, "joined_discord_id")?,
                    invite_code: source_optional_text(row, "invite_code")?,
                    account_created: source_timestamp(row, "account_created_at"),
                    joined_at: source_timestamp(row, "joined_at"),
                    eligible: flag(table, row, "is_eligible")?,
                    rewarded: flag(table, row, "is_rewarded")?,
                    rewarded_at: source_timestamp(row, "rewarded_at"),
                    matures: source_timestamp(row, "matured_at"),
                    left: flag(table, row, "has_left")?,
                    left_at: source_timestamp(row, "left_at"),
                    leave_count: optional_i64(table, row, "leave_count")?.unwrap_or(0),
                    rejoin_count: optional_i64(table, row, "rejoin_count")?.unwrap_or(0),
                    revoked_at: source_timestamp(row, "revoked_at"),
                    revoked_credits: optional_i64(table, row, "revoked_credits")?.unwrap_or(0),
                    last_seen: source_timestamp(row, "last_seen_at"),
                    created: source_timestamp(row, "created_at"),
                })
            })
            .await?;
    }
    extras.discord = DiscordRewards {
        per_invite: env_number("DISCORD_CREDITS_PER_INVITE", 50),
        per_boost: [
            env_number("DISCORD_CREDITS_PER_BOOST_1", 500),
            env_number("DISCORD_CREDITS_PER_BOOST_2", 1250),
        ],
        maturity_hours: env_number("DISCORD_INVITE_MATURITY_HOURS", 6).clamp(0, 24 * 365),
    };

    if context.has_table("cloudflare_domains").await? {
        extras.domains = context
            .map_table("cloudflare_domains", None, |_, row| {
                Ok(SourceDomain {
                    id: source_i64(row, "id")?,
                    name: source_text(row, "name")?,
                    zone_id: source_optional_text(row, "cloudflare_id")?,
                    created: source_timestamp(row, "created_at"),
                })
            })
            .await?;
    }
    if context.has_table("subdomains").await? {
        extras.subdomains = context
            .map_table("subdomains", None, |table, row| {
                Ok(SourceSubdomain {
                    id: source_i64(row, "id")?,
                    name: source_text(row, "name")?,
                    record_type: source_optional_text(row, "record_type")?,
                    token_cost: optional_i64(table, row, "token_cost")?.unwrap_or(0),
                    records: source_optional_text(row, "cloudflare_id")?,
                    domain_id: source_i64(row, "domain_id")?,
                    server_id: source_i64(row, "server_id")?,
                    created: source_timestamp(row, "created_at"),
                })
            })
            .await?;
    }

    if context.has_table("plugged_host_devboxes").await? {
        let mut devboxes = SourceDevboxes::default();
        for row in &context.table("plugged_host_devboxes", None).await? {
            if let Some(server_id) = source_optional_i64(row, "server_id")? {
                devboxes.server_ids.insert(server_id);
            }
            if let Some(mount_id) = source_optional_i64(row, "mount_id")? {
                devboxes.mount_ids.insert(mount_id);
            }
        }
        extras.devboxes = Some(devboxes);
    }

    extras.limbo = super::limbo::read(context).await?;

    Ok(extras)
}

/// A node's role, tags, SRV target and overallocation from a Pelican `nodes` row.
pub(super) fn node_placement(
    table: &SourceColumns,
    row: &SourceRow,
) -> Result<super::mapping::NodePlacement, anyhow::Error> {
    Ok(super::mapping::NodePlacement {
        role: None,
        tags: super::mapping::parse_tags(source_optional_text(row, "tags")?.as_deref()),
        srv_target: source_optional_text(row, "srv_target")?,
        memory_overallocate: optional_i64(table, row, "memory_overallocate")?,
        disk_overallocate: optional_i64(table, row, "disk_overallocate")?,
    })
}

/// Leaves the Pelican devbox plugin's devboxes out of the import: their hidden servers with
/// everything that hangs off them (databases, variables, backups, subusers, schedules, mounts;
/// their allocations stay as free allocations of the node), the mounts that gave them their game
/// server's files, the devbox egg (and its nest) when no other server uses it, and the system
/// account that owned them when it owns and shares nothing else. The panel makes devboxes anew
/// on their first start (docs/features/devboxes.md).
pub fn leave_out_devboxes(mut source: SourceData) -> SourceData {
    let Some(devboxes) = source.extras.devboxes.take() else {
        return source;
    };

    let servers: HashSet<i64> = source
        .servers
        .iter()
        .filter(|server| devboxes.server_ids.contains(&server.id))
        .map(|server| server.id)
        .collect();
    let mounts: HashSet<i64> = source
        .mounts
        .iter()
        .filter(|mount| devboxes.mount_ids.contains(&mount.id))
        .map(|mount| mount.id)
        .collect();
    if servers.is_empty() && mounts.is_empty() {
        return source;
    }

    let devbox_eggs: HashSet<i64> = source
        .servers
        .iter()
        .filter(|server| servers.contains(&server.id))
        .map(|server| server.egg_id)
        .collect();
    let owners: HashSet<i64> = source
        .servers
        .iter()
        .filter(|server| servers.contains(&server.id))
        .map(|server| server.owner_id)
        .collect();

    source
        .servers
        .retain(|server| !servers.contains(&server.id));
    let schedules: HashSet<i64> = source
        .schedules
        .iter()
        .filter(|schedule| servers.contains(&schedule.server_id))
        .map(|schedule| schedule.id)
        .collect();
    source
        .schedules
        .retain(|schedule| !schedules.contains(&schedule.id));
    source
        .tasks
        .retain(|task| !schedules.contains(&task.schedule_id));
    source
        .databases
        .retain(|database| !servers.contains(&database.server_id));
    source.server_variables.retain(|variable| {
        variable
            .server_id
            .is_none_or(|server_id| !servers.contains(&server_id))
    });
    source
        .backups
        .retain(|backup| !servers.contains(&backup.server_id));
    source
        .subusers
        .retain(|subuser| !servers.contains(&subuser.server_id));
    for allocation in &mut source.allocations {
        if allocation
            .server_id
            .is_some_and(|server_id| servers.contains(&server_id))
        {
            allocation.server_id = None;
        }
    }
    source
        .extras
        .servers
        .retain(|server| !servers.contains(&server.id));
    source
        .extras
        .subdomains
        .retain(|subdomain| !servers.contains(&subdomain.server_id));

    // the devbox egg, unless a kept server runs it or a kept egg inherits from it
    let used: HashSet<i64> = source
        .servers
        .iter()
        .map(|server| server.egg_id)
        .chain(
            source
                .eggs
                .iter()
                .filter(|egg| !devbox_eggs.contains(&egg.id))
                .flat_map(|egg| [egg.config_from, egg.copy_script_from])
                .flatten(),
        )
        .collect();
    let eggs: HashSet<i64> = devbox_eggs
        .into_iter()
        .filter(|egg| !used.contains(egg))
        .collect();
    let emptied_nests: HashSet<i64> = source
        .eggs
        .iter()
        .filter(|egg| eggs.contains(&egg.id))
        .map(|egg| egg.nest_id)
        .collect();
    source.eggs.retain(|egg| !eggs.contains(&egg.id));
    source
        .egg_variables
        .retain(|variable| !eggs.contains(&variable.egg_id));
    let nests_in_use: HashSet<i64> = source.eggs.iter().map(|egg| egg.nest_id).collect();
    source
        .nests
        .retain(|nest| !emptied_nests.contains(&nest.id) || nests_in_use.contains(&nest.id));

    source.mounts.retain(|mount| !mounts.contains(&mount.id));
    source
        .egg_mounts
        .retain(|link| !mounts.contains(&link.mount_id) && !eggs.contains(&link.target_id));
    source
        .node_mounts
        .retain(|link| !mounts.contains(&link.mount_id));
    source
        .server_mounts
        .retain(|link| !mounts.contains(&link.mount_id) && !servers.contains(&link.target_id));

    // the system account, when it owns and shares nothing else
    let still_used: HashSet<i64> = source
        .servers
        .iter()
        .map(|server| server.owner_id)
        .chain(source.subusers.iter().map(|subuser| subuser.user_id))
        .collect();
    let users: HashSet<i64> = source
        .users
        .iter()
        .filter(|user| owners.contains(&user.id) && !user.admin && !still_used.contains(&user.id))
        .map(|user| user.id)
        .collect();
    source.users.retain(|user| !users.contains(&user.id));
    source.ssh_keys.retain(|key| !users.contains(&key.user_id));

    source.extras.notes.push((
        "servers",
        format!(
            "devboxes were left out: {} hidden server(s), {} mount(s), {} egg(s) and {} system account(s); the panel makes devboxes anew",
            servers.len(),
            mounts.len(),
            eggs.len(),
            users.len()
        ),
    ));

    source
}

pub struct PlanNoprofitLink {
    pub user_uuid: uuid::Uuid,
    pub account_id: i64,
    pub email: String,
    pub username: String,
}

pub struct PlanTokens {
    pub user_uuid: uuid::Uuid,
    pub tokens: i64,
    pub earned: i64,
    pub daily_granted: Option<NaiveDate>,
}

pub struct PlanCredits {
    pub user_uuid: uuid::Uuid,
    pub credits: i64,
    pub extra_cpu: i32,
    pub extra_memory: i32,
    pub extra_disk: i32,
}

pub struct PlanTransaction {
    pub user_uuid: uuid::Uuid,
    pub amount: i64,
    pub balance_after: i64,
    pub kind: String,
    pub description: String,
    pub data: serde_json::Value,
    pub created: DateTime<Utc>,
}

pub struct PlanMembership {
    pub user_uuid: uuid::Uuid,
    pub plus_since: Option<DateTime<Utc>>,
    pub plusplus_until: Option<DateTime<Utc>>,
    pub plusplus_purchases: i32,
    pub plusplus_settled: Option<DateTime<Utc>>,
    pub created: DateTime<Utc>,
}

pub struct PlanDiscordMember {
    pub discord_id: String,
    pub user_uuid: Option<uuid::Uuid>,
    pub username: Option<String>,
    pub avatar: Option<String>,
    pub in_guild: bool,
    pub premium_since: Option<DateTime<Utc>>,
    pub boosts: i16,
    pub invite_debt: i64,
    pub checked: Option<DateTime<Utc>>,
    pub created: DateTime<Utc>,
}

pub struct PlanBoostPayout {
    pub discord_id: String,
    pub user_uuid: Option<uuid::Uuid>,
    pub level: i16,
    pub credits: i64,
}

pub struct PlanDiscordInvite {
    pub inviter: String,
    pub joined: String,
    pub invite_code: Option<String>,
    pub account_created: DateTime<Utc>,
    pub joined_at: DateTime<Utc>,
    pub eligible: bool,
    pub matures: DateTime<Utc>,
    pub rewarded: Option<DateTime<Utc>>,
    pub credits: i64,
    pub departed: Option<DateTime<Utc>>,
    pub leave_count: i32,
    pub rejoin_count: i32,
    pub revoked: Option<DateTime<Utc>>,
    pub revoked_credits: i64,
    pub last_seen: Option<DateTime<Utc>>,
    pub created: DateTime<Utc>,
}

pub struct PlanDomain {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub name: String,
    pub zone_id: Option<String>,
    pub created: DateTime<Utc>,
}

pub struct PlanSubdomain {
    pub source_id: i64,
    pub server_uuid: uuid::Uuid,
    pub domain_uuid: uuid::Uuid,
    pub name: String,
    pub record_type: String,
    pub records: serde_json::Value,
    pub token_cost: i32,
    pub created: DateTime<Utc>,
}

#[derive(Default)]
pub struct PlanExtras {
    pub noprofit_links: Vec<PlanNoprofitLink>,
    pub tokens: Vec<PlanTokens>,
    pub credits: Vec<PlanCredits>,
    pub token_transactions: Vec<PlanTransaction>,
    pub credit_transactions: Vec<PlanTransaction>,
    pub memberships: Vec<PlanMembership>,
    pub user_settings: Vec<(uuid::Uuid, serde_json::Value)>,
    pub discord_members: Vec<PlanDiscordMember>,
    pub boost_payouts: Vec<PlanBoostPayout>,
    pub discord_invites: Vec<PlanDiscordInvite>,
    pub domains: Vec<PlanDomain>,
    pub subdomains: Vec<PlanSubdomain>,
    pub subdomain_limits: Vec<(uuid::Uuid, i32)>,
    pub icons: Vec<(uuid::Uuid, PathBuf)>,
    pub limbo: Vec<super::limbo::PlanLimbo>,
}

impl PlanExtras {
    /// What will be written, leaving out what the source did not have.
    pub fn counts(&self) -> Vec<(&'static str, usize)> {
        [
            ("noprofit.space links", self.noprofit_links.len()),
            ("token balances", self.tokens.len()),
            ("credit balances", self.credits.len()),
            ("token transactions", self.token_transactions.len()),
            ("credit transactions", self.credit_transactions.len()),
            ("memberships", self.memberships.len()),
            ("user settings", self.user_settings.len()),
            ("discord members", self.discord_members.len()),
            ("discord boost payouts", self.boost_payouts.len()),
            ("discord invites", self.discord_invites.len()),
            ("domains", self.domains.len()),
            ("subdomains", self.subdomains.len()),
            ("server subdomain limits", self.subdomain_limits.len()),
            ("server icons", self.icons.len()),
            ("limbo servers", self.limbo.len()),
        ]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .collect()
    }
}

fn truncated(value: &str, max: usize) -> String {
    value.trim().chars().take(max).collect()
}

fn clamp_i32(value: i64) -> i32 {
    value.clamp(0, i32::MAX as i64) as i32
}

/// A Discord account id: a snowflake, digits only.
fn discord_id(value: &str) -> Option<String> {
    let value = value.trim();

    ((1..=32).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit()))
        .then(|| value.to_string())
}

/// Calagopus' name of a Pelican token transaction type, the same where both use one name.
pub fn token_type(kind: &str) -> &str {
    match kind {
        "admin_adjustment" => shared::economy::TYPE_ADMIN,
        "limbo_restore" => shared::limbo::TYPE_RESTORE,
        "limbo_restore_refund" => shared::limbo::TYPE_REFUND,
        "subdomain_create" => shared::subdomains::TYPE_SUBDOMAIN,
        "subdomain_refund" => shared::subdomains::TYPE_REFUND,
        other => other,
    }
}

/// Calagopus' name of a Pelican credit transaction type, the same where both use one name.
pub fn credit_type(kind: &str) -> &str {
    match kind {
        "admin_adjustment" => shared::economy::TYPE_ADMIN,
        other => other,
    }
}

/// A transaction's details: the Pelican metadata when it is JSON, else an empty object.
fn transaction_data(metadata: Option<&str>) -> serde_json::Value {
    metadata
        .and_then(|metadata| serde_json::from_str::<serde_json::Value>(metadata).ok())
        .filter(|data| !data.is_null())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// A subdomain's Cloudflare record ids by key (`a`, `srv`, `srv_udp`, `srv_tcp`). Pelican keeps
/// them as that JSON object, or, from before it had SRV records, as the address record's id alone.
pub fn subdomain_records(raw: Option<&str>) -> serde_json::Value {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return serde_json::json!({});
    };

    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Object(records)) => serde_json::Value::Object(
            records
                .into_iter()
                .filter(|(_, id)| id.as_str().is_some_and(|id| !id.trim().is_empty()))
                .collect(),
        ),
        Ok(serde_json::Value::String(id)) if !id.trim().is_empty() => {
            serde_json::json!({ "a": id.trim() })
        }
        // an id of digits only reads as a number
        Ok(serde_json::Value::Number(_)) => serde_json::json!({ "a": raw }),
        Ok(_) => serde_json::json!({}),
        Err(_) => serde_json::json!({ "a": raw }),
    }
}

fn transactions(
    rows: &[SourceTransaction],
    users: &IdMap,
    table: &'static str,
    map_type: fn(&str) -> &str,
    now: DateTime<Utc>,
    notes: &mut Vec<(&'static str, String)>,
) -> Vec<PlanTransaction> {
    let mut planned = Vec::with_capacity(rows.len());

    for row in rows {
        let Some(user_uuid) = users.get(&row.user_id) else {
            notes.push((table, LEFT_OUT_USER.into()));
            continue;
        };

        planned.push(PlanTransaction {
            user_uuid: *user_uuid,
            amount: row.amount,
            balance_after: row.balance_after.max(0),
            kind: truncated(map_type(row.kind.trim()), 64),
            description: truncated(&row.description, 255),
            data: transaction_data(row.metadata.as_deref()),
            created: row.created.unwrap_or(now),
        });
    }

    planned
}

/// Plans the extras for the users and servers being imported (`users`, `servers`: source id to
/// uuid) and returns notes for the summary.
pub fn plan(
    source: &SourceData,
    users: &IdMap,
    servers: &IdMap,
    target: &mut Plan,
) -> Vec<(&'static str, String)> {
    let extras = &source.extras;
    let now = Utc::now();
    let mut notes = extras.notes.clone();
    let mut planned = PlanExtras::default();

    let plan_users: HashMap<uuid::Uuid, &PlanUser> =
        target.users.iter().map(|user| (user.uuid, user)).collect();
    let mut accounts: HashSet<i64> = HashSet::new();
    for user in &extras.users {
        let Some(user_uuid) = users.get(&user.id) else {
            continue;
        };

        if let Some(account_id) = user.noprofit_account_id.filter(|id| *id > 0)
            && let Some(plan_user) = plan_users.get(user_uuid)
        {
            if accounts.insert(account_id) {
                planned.noprofit_links.push(PlanNoprofitLink {
                    user_uuid: *user_uuid,
                    account_id,
                    email: plan_user.email.clone(),
                    username: plan_user.username.clone(),
                });
            } else {
                notes.push((
                    "user_noprofit_accounts",
                    "a noprofit.space account linked to more than one user was linked to the first only"
                        .into(),
                ));
            }
        }

        if let Some(size) = user.console_font_size {
            planned.user_settings.push((
                *user_uuid,
                serde_json::json!({ "console::font_size": size }),
            ));
        }
    }

    for row in &extras.tokens {
        let Some(user_uuid) = users.get(&row.user_id) else {
            notes.push(("user_tokens", LEFT_OUT_USER.into()));
            continue;
        };
        planned.tokens.push(PlanTokens {
            user_uuid: *user_uuid,
            tokens: row.tokens.max(0),
            earned: row.earned.max(0),
            daily_granted: row.daily_granted,
        });
    }
    for row in &extras.credits {
        let Some(user_uuid) = users.get(&row.user_id) else {
            notes.push(("user_credits", LEFT_OUT_USER.into()));
            continue;
        };
        planned.credits.push(PlanCredits {
            user_uuid: *user_uuid,
            credits: row.credits.max(0),
            extra_cpu: clamp_i32(row.extra_cpu),
            extra_memory: clamp_i32(row.extra_memory),
            extra_disk: clamp_i32(row.extra_disk),
        });
    }
    planned.token_transactions = transactions(
        &extras.token_transactions,
        users,
        "token_transactions",
        token_type,
        now,
        &mut notes,
    );
    planned.credit_transactions = transactions(
        &extras.credit_transactions,
        users,
        "credit_transactions",
        credit_type,
        now,
        &mut notes,
    );

    for row in &extras.memberships {
        let Some(user_uuid) = users.get(&row.user_id) else {
            notes.push(("user_memberships", LEFT_OUT_USER.into()));
            continue;
        };
        planned.memberships.push(PlanMembership {
            user_uuid: *user_uuid,
            plus_since: row.plus_since,
            plusplus_until: row.plusplus_until,
            plusplus_purchases: clamp_i32(row.plusplus_purchases),
            plusplus_settled: row.plusplus_settled,
            created: row.created.unwrap_or(now),
        });
    }

    let mut members: HashSet<String> = HashSet::new();
    for link in &extras.discord_links {
        let Some(id) = link.discord_id.as_deref().and_then(discord_id) else {
            notes.push((
                "discord_links",
                "links without a Discord account id were left out".into(),
            ));
            continue;
        };
        if !members.insert(id.clone()) {
            notes.push((
                "discord_links",
                "a Discord account linked more than once was imported once".into(),
            ));
            continue;
        }
        let user_uuid = users.get(&link.user_id).copied();
        if user_uuid.is_none() {
            notes.push((
                "discord_links",
                "Discord accounts of users that were not imported were kept without a user".into(),
            ));
        }

        planned.discord_members.push(PlanDiscordMember {
            discord_id: id.clone(),
            user_uuid,
            username: link
                .username
                .as_deref()
                .map(|username| truncated(username, 64))
                .filter(|username| !username.is_empty()),
            avatar: link
                .avatar
                .as_deref()
                .map(str::trim)
                .filter(|avatar| !avatar.is_empty() && avatar.chars().count() <= 64)
                .map(str::to_string),
            in_guild: link.in_guild,
            premium_since: link.boosted_since.filter(|_| link.boosting),
            boosts: match (link.boosting, link.boost_count) {
                (false, _) => 0,
                (true, count) if count >= 2 => 2,
                (true, _) => 1,
            },
            invite_debt: link.invite_debt.max(0),
            checked: link.verified,
            created: link.created.unwrap_or(now),
        });
        for level in 1..=link.boosts_rewarded.clamp(0, 2) {
            planned.boost_payouts.push(PlanBoostPayout {
                discord_id: id.clone(),
                user_uuid,
                level: level as i16,
                credits: extras.discord.boost_credits(level),
            });
        }
    }

    let mut joined: HashSet<String> = HashSet::new();
    for invite in &extras.discord_invites {
        let (Some(inviter), Some(joined_id)) =
            (discord_id(&invite.inviter), discord_id(&invite.joined))
        else {
            notes.push((
                "discord_invited_members",
                "invites without valid Discord account ids were left out".into(),
            ));
            continue;
        };
        if !joined.insert(joined_id.clone()) {
            notes.push((
                "discord_invited_members",
                "a member who joined more than once was imported once".into(),
            ));
            continue;
        }
        let Some(joined_at) = invite.joined_at.or(invite.created) else {
            notes.push((
                "discord_invited_members",
                "invites without a join time were left out".into(),
            ));
            continue;
        };

        let matures = invite
            .matures
            .unwrap_or_else(|| joined_at + chrono::Duration::hours(extras.discord.maturity_hours));
        let rewarded = invite
            .rewarded
            .then(|| invite.rewarded_at.unwrap_or(matures));

        planned.discord_invites.push(PlanDiscordInvite {
            inviter,
            joined: joined_id,
            invite_code: invite
                .invite_code
                .as_deref()
                .map(str::trim)
                .filter(|code| !code.is_empty() && code.chars().count() <= 32)
                .map(str::to_string),
            account_created: invite.account_created.unwrap_or(joined_at),
            joined_at,
            eligible: invite.eligible,
            matures,
            rewarded,
            credits: if rewarded.is_some() {
                extras.discord.per_invite.max(0)
            } else {
                0
            },
            departed: invite.left.then(|| invite.left_at.unwrap_or(joined_at)),
            leave_count: clamp_i32(invite.leave_count),
            rejoin_count: clamp_i32(invite.rejoin_count),
            revoked: invite.revoked_at,
            revoked_credits: invite.revoked_credits.max(0),
            last_seen: invite.last_seen,
            created: invite.created.unwrap_or(joined_at),
        });
    }

    let mut domains: IdMap = IdMap::new();
    let mut domain_names: HashSet<String> = HashSet::new();
    for domain in &extras.domains {
        let name = domain.name.trim().trim_end_matches('.').to_lowercase();
        if name.is_empty() || name.len() > 253 || !domain_names.insert(name.clone()) {
            notes.push((
                "cloudflare_domains",
                "domains with an empty, too long or repeated name were left out".into(),
            ));
            continue;
        }

        let uuid = uuid::Uuid::new_v4();
        domains.insert(domain.id, uuid);
        planned.domains.push(PlanDomain {
            source_id: domain.id,
            uuid,
            name,
            zone_id: domain
                .zone_id
                .as_deref()
                .map(str::trim)
                .filter(|zone| !zone.is_empty() && zone.len() <= 64)
                .map(str::to_string),
            created: domain.created.unwrap_or(now),
        });
    }

    let mut subdomain_names: HashSet<(uuid::Uuid, String)> = HashSet::new();
    for subdomain in &extras.subdomains {
        let (Some(server_uuid), Some(domain_uuid)) = (
            servers.get(&subdomain.server_id),
            domains.get(&subdomain.domain_id),
        ) else {
            notes.push((
                "subdomains",
                "subdomains of servers or domains that were not imported were left out".into(),
            ));
            continue;
        };

        let name = subdomain.name.trim().to_lowercase();
        if name.is_empty()
            || name.len() > 63
            || !subdomain_names.insert((*domain_uuid, name.clone()))
        {
            notes.push((
                "subdomains",
                "subdomains with an empty, too long or repeated name were left out".into(),
            ));
            continue;
        }

        let record_type = subdomain
            .record_type
            .as_deref()
            .unwrap_or("A")
            .trim()
            .to_uppercase();
        let record_type = if matches!(record_type.as_str(), "A" | "AAAA" | "SRV") {
            record_type
        } else {
            notes.push((
                "subdomains",
                "record types other than A, AAAA and SRV were imported as A".into(),
            ));
            "A".to_string()
        };

        planned.subdomains.push(PlanSubdomain {
            source_id: subdomain.id,
            server_uuid: *server_uuid,
            domain_uuid: *domain_uuid,
            name,
            record_type,
            records: subdomain_records(subdomain.records.as_deref()),
            token_cost: clamp_i32(subdomain.token_cost),
            created: subdomain.created.unwrap_or(now),
        });
    }

    for server in &extras.servers {
        let Some(server_uuid) = servers.get(&server.id) else {
            continue;
        };

        // Pelican reads 0 or less as "the default"; Calagopus keeps the default as empty
        if let Some(limit) = server.subdomain_limit.filter(|limit| *limit > 0) {
            planned
                .subdomain_limits
                .push((*server_uuid, clamp_i32(limit)));
        }
        if let Some(icon) = &server.icon {
            planned.icons.push((*server_uuid, icon.clone()));
        }
    }

    target.extras = planned;

    notes
}

/// Writes the extras and the location and node fields of mapping.rs, inside the import's
/// transaction. Rows that already exist in a panel imported into with `--force` are kept.
pub async fn write(tx: &mut sqlx::PgConnection, plan: &Plan) -> Result<(), anyhow::Error> {
    for location in &plan.locations {
        sqlx::query(
            "UPDATE locations SET flag = $2, tier = $3, sort = $4, public = $5 WHERE uuid = $1",
        )
        .bind(location.uuid)
        .bind(&location.placement.flag)
        .bind(&location.placement.tier)
        .bind(location.placement.sort)
        .bind(location.placement.public)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("failed to write location {:?}", location.name))?;
    }

    for node in &plan.nodes {
        sqlx::query("UPDATE nodes SET role = $2, tags = $3, srv_target = $4 WHERE uuid = $1")
            .bind(node.uuid)
            .bind(node.role.as_str())
            .bind(&node.tags)
            .bind(&node.srv_target)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("failed to write node {:?}", node.name))?;
    }

    let extras = &plan.extras;

    for link in &extras.noprofit_links {
        sqlx::query(
            "INSERT INTO user_noprofit_accounts (user_uuid, account_id, email, username)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING",
        )
        .bind(link.user_uuid)
        .bind(link.account_id)
        .bind(&link.email)
        .bind(&link.username)
        .execute(&mut *tx)
        .await
        .context("failed to write a noprofit.space link")?;
    }
    tracing::info!("wrote {} noprofit.space links", extras.noprofit_links.len());

    for tokens in &extras.tokens {
        sqlx::query(
            "UPDATE users SET tokens = $2, tokens_earned = $3, tokens_daily_granted = $4 WHERE uuid = $1",
        )
        .bind(tokens.user_uuid)
        .bind(tokens.tokens)
        .bind(tokens.earned)
        .bind(tokens.daily_granted)
        .execute(&mut *tx)
        .await
        .context("failed to write a token balance")?;
    }
    for credits in &extras.credits {
        sqlx::query(
            "UPDATE users SET credits = $2, extra_cpu = $3, extra_memory = $4, extra_disk = $5 WHERE uuid = $1",
        )
        .bind(credits.user_uuid)
        .bind(credits.credits)
        .bind(credits.extra_cpu)
        .bind(credits.extra_memory)
        .bind(credits.extra_disk)
        .execute(&mut *tx)
        .await
        .context("failed to write a credit balance")?;
    }

    for (table, rows) in [
        ("user_token_transactions", &extras.token_transactions),
        ("user_credit_transactions", &extras.credit_transactions),
    ] {
        for row in rows {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {table} (user_uuid, amount, balance_after, type, description, data, created)
                VALUES ($1, $2, $3, $4, $5, $6, $7)"
            )))
            .bind(row.user_uuid)
            .bind(row.amount)
            .bind(row.balance_after)
            .bind(&row.kind)
            .bind(&row.description)
            .bind(&row.data)
            .bind(row.created.naive_utc())
            .execute(&mut *tx)
            .await
            .with_context(|| format!("failed to write a row of {table}"))?;
        }
        tracing::info!("wrote {} {table}", rows.len());
    }

    for membership in &extras.memberships {
        sqlx::query(
            "INSERT INTO user_memberships (user_uuid, plus_since, plusplus_until, plusplus_purchases, plusplus_settled, created)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT DO NOTHING",
        )
        .bind(membership.user_uuid)
        .bind(membership.plus_since.map(|time| time.naive_utc()))
        .bind(membership.plusplus_until.map(|time| time.naive_utc()))
        .bind(membership.plusplus_purchases)
        .bind(membership.plusplus_settled.map(|time| time.naive_utc()))
        .bind(membership.created.naive_utc())
        .execute(&mut *tx)
        .await
        .context("failed to write a membership")?;
    }

    for (user_uuid, settings) in &extras.user_settings {
        sqlx::query(
            "INSERT INTO user_settings (user_uuid, settings) VALUES ($1, $2)
            ON CONFLICT (user_uuid) DO UPDATE SET settings = user_settings.settings || EXCLUDED.settings",
        )
        .bind(user_uuid)
        .bind(settings)
        .execute(&mut *tx)
        .await
        .context("failed to write user settings")?;
    }

    for member in &extras.discord_members {
        sqlx::query(
            "INSERT INTO discord_members (discord_id, user_uuid, username, avatar, in_guild, premium_since, boosts, invite_debt, checked, created)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT DO NOTHING",
        )
        .bind(&member.discord_id)
        .bind(member.user_uuid)
        .bind(&member.username)
        .bind(&member.avatar)
        .bind(member.in_guild)
        .bind(member.premium_since.map(|time| time.naive_utc()))
        .bind(member.boosts)
        .bind(member.invite_debt)
        .bind(member.checked.map(|time| time.naive_utc()))
        .bind(member.created.naive_utc())
        .execute(&mut *tx)
        .await
        .context("failed to write a Discord member")?;
    }
    for payout in &extras.boost_payouts {
        sqlx::query(
            "INSERT INTO discord_boost_payouts (discord_id, user_uuid, level, credits)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING",
        )
        .bind(&payout.discord_id)
        .bind(payout.user_uuid)
        .bind(payout.level)
        .bind(payout.credits)
        .execute(&mut *tx)
        .await
        .context("failed to write a Discord boost payout")?;
    }
    for invite in &extras.discord_invites {
        sqlx::query(
            "INSERT INTO discord_invites (
                inviter_discord_id, joined_discord_id, invite_code, account_created, joined, eligible,
                matures, rewarded, credits, departed, leave_count, rejoin_count, revoked,
                revoked_credits, last_seen, created
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT DO NOTHING",
        )
        .bind(&invite.inviter)
        .bind(&invite.joined)
        .bind(&invite.invite_code)
        .bind(invite.account_created.naive_utc())
        .bind(invite.joined_at.naive_utc())
        .bind(invite.eligible)
        .bind(invite.matures.naive_utc())
        .bind(invite.rewarded.map(|time| time.naive_utc()))
        .bind(invite.credits)
        .bind(invite.departed.map(|time| time.naive_utc()))
        .bind(invite.leave_count)
        .bind(invite.rejoin_count)
        .bind(invite.revoked.map(|time| time.naive_utc()))
        .bind(invite.revoked_credits)
        .bind(invite.last_seen.map(|time| time.naive_utc()))
        .bind(invite.created.naive_utc())
        .execute(&mut *tx)
        .await
        .context("failed to write a Discord invite")?;
    }
    tracing::info!(
        "wrote {} Discord members and {} invites",
        extras.discord_members.len(),
        extras.discord_invites.len()
    );

    // a domain the panel already has keeps its row, and the subdomains go under it
    let mut domains: HashMap<uuid::Uuid, uuid::Uuid> = HashMap::new();
    for domain in &extras.domains {
        sqlx::query(
            "INSERT INTO domains (uuid, name, zone_id, created) VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING",
        )
        .bind(domain.uuid)
        .bind(&domain.name)
        .bind(&domain.zone_id)
        .bind(domain.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| {
            format!(
                "failed to write domain #{} {:?}",
                domain.source_id, domain.name
            )
        })?;

        let uuid: uuid::Uuid = sqlx::query_scalar("SELECT uuid FROM domains WHERE name = $1")
            .bind(&domain.name)
            .fetch_one(&mut *tx)
            .await
            .with_context(|| format!("failed to read domain {:?}", domain.name))?;
        domains.insert(domain.uuid, uuid);
    }
    for subdomain in &extras.subdomains {
        sqlx::query(
            "INSERT INTO server_subdomains (server_uuid, domain_uuid, name, record_type, records, token_cost, created)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT DO NOTHING",
        )
        .bind(subdomain.server_uuid)
        .bind(
            domains
                .get(&subdomain.domain_uuid)
                .copied()
                .unwrap_or(subdomain.domain_uuid),
        )
        .bind(&subdomain.name)
        .bind(&subdomain.record_type)
        .bind(&subdomain.records)
        .bind(subdomain.token_cost)
        .bind(subdomain.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| format!("failed to write subdomain #{}", subdomain.source_id))?;
    }
    for (server_uuid, limit) in &extras.subdomain_limits {
        sqlx::query("UPDATE servers SET subdomain_limit = $2 WHERE uuid = $1")
            .bind(server_uuid)
            .bind(limit)
            .execute(&mut *tx)
            .await
            .context("failed to write a subdomain limit")?;
    }
    tracing::info!(
        "wrote {} domains and {} subdomains",
        extras.domains.len(),
        extras.subdomains.len()
    );

    Ok(())
}

/// Stores the server icons once the import is written: each file is turned into a square WebP
/// like an upload (`models::server::icon::transcode`) and stored under the server's path. An icon
/// that cannot be read, decoded or stored is reported and skipped, and its server shows its game's
/// icon; nothing else is undone.
pub async fn store_icons(
    database: &shared::database::Database,
    settings: Arc<shared::settings::Settings>,
    icons: &[(uuid::Uuid, PathBuf)],
) {
    use rand::distr::SampleString;
    use shared::models::ByUuid;

    if icons.is_empty() {
        return;
    }

    let storage = shared::storage::Storage::new(settings);
    let mut stored = 0;

    for (server_uuid, file) in icons {
        let result: Result<(), anyhow::Error> = async {
            let data = tokio::fs::read(file).await?;
            let data = shared::models::server::icon::transcode(data).await?;
            let path = shared::models::server::icon::stored_path(
                *server_uuid,
                &rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 8),
            );

            storage
                .store_with_cache_control(
                    &path,
                    data.as_slice(),
                    "image/webp",
                    Some("public, max-age=31536000, immutable"),
                )
                .await?;
            sqlx::query("UPDATE servers SET icon = $2 WHERE uuid = $1 AND icon IS NULL")
                .bind(server_uuid)
                .bind(&path)
                .execute(database.write())
                .await?;
            shared::models::server::Server::invalidate_cached(database, *server_uuid).await;

            Ok::<(), anyhow::Error>(())
        }
        .await;

        match result {
            Ok(()) => stored += 1,
            Err(err) => eprintln!(
                "{} the icon {} of server {server_uuid} was not imported: {err:#}",
                "warning:".yellow(),
                file.display()
            ),
        }
    }

    tracing::info!("stored {stored} of {} server icons", icons.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_font_sizes_are_read_from_customization() {
        let doubled = serde_json::to_string(
            r#"{"console_font":"monospace","console_font_size":15,"console_rows":30}"#,
        )
        .unwrap();
        assert_eq!(console_font_size(Some(&doubled)), Some(15));
        assert_eq!(
            console_font_size(Some(r#"{"console_font_size":"18"}"#)),
            Some(18)
        );
        // the default, nonsense and nothing
        assert_eq!(console_font_size(Some(r#"{"console_font_size":14}"#)), None);
        assert_eq!(
            console_font_size(Some(r#"{"console_font_size":400}"#)),
            None
        );
        assert_eq!(console_font_size(Some(r#"{"servers_per_page":40}"#)), None);
        assert_eq!(console_font_size(Some("[]")), None);
        assert_eq!(console_font_size(Some("not json")), None);
        assert_eq!(console_font_size(None), None);
    }

    #[test]
    fn subdomain_records_keep_their_ids() {
        assert_eq!(
            subdomain_records(Some(r#"{"a":"id-a","srv":"id-srv"}"#)),
            serde_json::json!({ "a": "id-a", "srv": "id-srv" })
        );
        assert_eq!(
            subdomain_records(Some(r#"{"a":"id-a","srv_udp":"","srv_tcp":null}"#)),
            serde_json::json!({ "a": "id-a" })
        );
        // from before SRV records: the address record's id alone
        assert_eq!(
            subdomain_records(Some("0123456789abcdef")),
            serde_json::json!({ "a": "0123456789abcdef" })
        );
        assert_eq!(
            subdomain_records(Some(r#""quoted-id""#)),
            serde_json::json!({ "a": "quoted-id" })
        );
        assert_eq!(
            subdomain_records(Some("123456")),
            serde_json::json!({ "a": "123456" })
        );
        assert_eq!(subdomain_records(Some(" ")), serde_json::json!({}));
        assert_eq!(subdomain_records(None), serde_json::json!({}));
    }

    #[test]
    fn transaction_types_use_calagopus_names() {
        assert_eq!(token_type("admin_adjustment"), "admin");
        assert_eq!(token_type("limbo_restore"), "limbo");
        assert_eq!(token_type("limbo_restore_refund"), "limbo_refund");
        assert_eq!(token_type("subdomain_create"), "subdomain");
        assert_eq!(token_type("subdomain_refund"), "subdomain_refund");
        assert_eq!(token_type("daily"), "daily");
        assert_eq!(token_type("earn:example"), "earn:example");
        assert_eq!(credit_type("admin_adjustment"), "admin");
        assert_eq!(credit_type("discord_invite"), "discord_invite");
        assert_eq!(credit_type("resource_purchase"), "resource_purchase");
    }

    #[test]
    fn transaction_data_is_json() {
        assert_eq!(
            transaction_data(Some(r#"{"server":1}"#)),
            serde_json::json!({ "server": 1 })
        );
        assert_eq!(transaction_data(Some("null")), serde_json::json!({}));
        assert_eq!(transaction_data(Some("not json")), serde_json::json!({}));
        assert_eq!(transaction_data(None), serde_json::json!({}));
    }

    #[test]
    fn boost_credits_are_per_level() {
        let rewards = DiscordRewards::default();
        assert_eq!(rewards.boost_credits(1), 500);
        assert_eq!(rewards.boost_credits(2), 750);
        assert_eq!(rewards.boost_credits(3), 0);

        let odd = DiscordRewards {
            per_boost: [800, 500],
            ..Default::default()
        };
        assert_eq!(odd.boost_credits(2), 0);
    }

    #[test]
    fn discord_ids_are_snowflakes() {
        assert_eq!(
            discord_id(" 123456789012345678 ").as_deref(),
            Some("123456789012345678")
        );
        assert_eq!(discord_id(""), None);
        assert_eq!(discord_id("12a"), None);
        assert_eq!(discord_id(&"1".repeat(33)), None);
    }

    #[test]
    fn icon_files_are_found_by_server_uuid() {
        let directory = std::env::temp_dir().join(format!("import-icons-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let server = uuid::Uuid::new_v4();
        std::fs::write(directory.join(format!("{server}.webp")), b"x").unwrap();
        std::fs::write(directory.join(format!("{server}.jpg")), b"x").unwrap();

        // the Pelican panel's order: png, jpg, webp
        assert_eq!(
            icon_file(&directory, &server.to_string()),
            Some(directory.join(format!("{server}.jpg")))
        );
        assert_eq!(
            icon_file(&directory, &uuid::Uuid::new_v4().to_string()),
            None
        );
        assert_eq!(icon_file(&directory, "../../etc/passwd"), None);

        std::fs::remove_dir_all(&directory).unwrap();
    }
}
