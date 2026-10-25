use chrono::{DateTime, Utc};
use compact_str::CompactString;
use indexmap::IndexMap;
use shared::models::{
    nest_egg::{
        NestEggConfigScript, NestEggConfigStartup, NestEggConfigStop, ProcessConfigurationFile,
    },
    server::ServerStatus,
    server_backup::BackupDisk,
};
use std::collections::HashMap;

/// A source value that could not be read, kept so validation can point at it.
#[derive(Debug, Clone)]
pub struct Unreadable {
    pub value: String,
    pub reason: String,
}

impl Unreadable {
    pub fn new(value: impl Into<String>, reason: impl std::fmt::Display) -> Self {
        Self {
            value: value.into(),
            reason: reason.to_string(),
        }
    }
}

pub type Readable<T> = Result<T, Unreadable>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Pterodactyl,
    Pelican,
}

impl SourceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pterodactyl => "pterodactyl",
            Self::Pelican => "pelican",
        }
    }
}

pub struct SourceMail {
    pub host: CompactString,
    pub port: u16,
    pub username: Option<CompactString>,
    pub password: Option<CompactString>,
    pub start_tls: bool,
    pub from_address: CompactString,
    pub from_name: Option<CompactString>,
}

pub struct SourceSettings {
    pub app_url: CompactString,
    pub app_name: Option<CompactString>,
    pub mail: Option<SourceMail>,
}

pub struct SourceBackupConfig {
    pub id: i64,
    pub name: String,
    pub disk: BackupDisk,
    pub s3: Option<shared::models::backup_configuration::BackupConfigsS3>,
}

pub struct SourceUser {
    pub id: i64,
    pub uuid: Readable<uuid::Uuid>,
    pub external_id: Option<String>,
    pub username: String,
    pub email: String,
    pub name_first: Option<String>,
    pub name_last: Option<String>,
    pub password: String,
    pub admin: bool,
    pub totp_enabled: bool,
    pub totp_secret: Option<Readable<CompactString>>,
    pub language: Option<String>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceSshKey {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub public_key: String,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceLocation {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created: Option<DateTime<Utc>>,
    pub placement: super::mapping::LocationPlacement, // plugged.host: flag, tier, sort, public
}

pub struct SourceNode {
    pub id: i64,
    pub uuid: Readable<uuid::Uuid>,
    pub location_id: i64,
    pub backup_config_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub public: bool,
    pub maintenance_mode: bool,
    pub scheme: String,
    pub fqdn: String,
    pub daemon_listen: i64,
    pub daemon_connect: Option<i64>,
    pub daemon_sftp: i64,
    pub sftp_alias: Option<String>,
    pub memory: i64,
    pub disk: i64,
    pub token_id: String,
    pub token: Readable<CompactString>,
    pub created: Option<DateTime<Utc>>,
    pub placement: super::mapping::NodePlacement, // plugged.host: role, tags, SRV target, overallocation
}

pub struct SourceNest {
    pub id: i64,
    pub uuid: Option<Readable<uuid::Uuid>>,
    pub author: String,
    pub name: String,
    pub description: Option<String>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceEgg {
    pub id: i64,
    pub uuid: Readable<uuid::Uuid>,
    pub nest_id: i64,
    pub author: String,
    pub name: String,
    pub description: Option<String>,
    pub features: Option<String>,
    pub docker_images: Option<String>,
    pub file_denylist: Option<String>,
    pub config_files: Option<String>,
    pub config_startup: Option<String>,
    pub config_stop: Option<String>,
    pub config_from: Option<i64>,
    pub copy_script_from: Option<i64>,
    pub script_container: Option<String>,
    pub script_entry: Option<String>,
    pub script_install: Option<String>,
    /// Either the single Pterodactyl startup command or Pelican's named commands as JSON.
    pub startup: SourceEggStartup,
    pub force_outgoing_ip: bool,
    pub created: Option<DateTime<Utc>>,
    pub tag_features: Vec<CompactString>, // plugged.host: features from Pelican egg tags (mapping.rs)
}

pub enum SourceEggStartup {
    Single(Option<String>),
    Named(Option<String>),
}

pub struct SourceEggVariable {
    pub id: i64,
    pub egg_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub env_variable: String,
    pub default_value: Option<String>,
    pub user_viewable: bool,
    pub user_editable: bool,
    pub rules: Vec<CompactString>,
    pub order: i64,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceDatabaseHost {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub password: Readable<CompactString>,
    /// `None` means the host is not tied to nodes and serves the whole panel.
    pub node_ids: Option<Vec<i64>>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceServer {
    pub id: i64,
    pub uuid: Readable<uuid::Uuid>,
    pub external_id: Option<String>,
    pub node_id: i64,
    pub owner_id: i64,
    pub egg_id: i64,
    pub allocation_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub memory: i64,
    pub swap: i64,
    pub disk: i64,
    pub io: i64,
    pub cpu: i64,
    pub threads: Option<String>,
    pub startup: String,
    pub image: String,
    pub docker_labels: Option<String>,
    pub allocation_limit: Option<i64>,
    pub database_limit: Option<i64>,
    pub backup_limit: Option<i64>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceDatabase {
    pub id: i64,
    pub server_id: i64,
    pub database_host_id: i64,
    pub name: String,
    pub username: String,
    pub password: Readable<CompactString>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceServerVariable {
    pub id: i64,
    pub server_id: Option<i64>,
    pub variable_id: i64,
    pub value: Option<String>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceBackup {
    pub id: i64,
    pub uuid: Readable<uuid::Uuid>,
    pub server_id: i64,
    pub backup_config_id: Option<i64>,
    pub name: String,
    pub successful: bool,
    pub locked: bool,
    pub ignored_files: Option<String>,
    pub disk: String,
    pub checksum: Option<String>,
    pub bytes: i64,
    pub upload_id: Option<String>,
    pub completed: Option<DateTime<Utc>>,
    pub deleted: Option<DateTime<Utc>>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceSubuser {
    pub id: i64,
    pub user_id: i64,
    pub server_id: i64,
    pub permissions: Option<String>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceMount {
    pub id: i64,
    pub uuid: Readable<uuid::Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub target: String,
    pub read_only: bool,
    pub user_mountable: bool,
}

pub struct SourceMountLink {
    pub mount_id: i64,
    pub target_id: i64,
}

pub struct SourceSchedule {
    pub id: i64,
    pub server_id: i64,
    pub name: String,
    pub enabled: bool,
    pub only_when_online: bool,
    pub cron_minute: String,
    pub cron_hour: String,
    pub cron_day_of_month: String,
    pub cron_month: String,
    pub cron_day_of_week: String,
    pub last_run: Option<DateTime<Utc>>,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceTask {
    pub id: i64,
    pub schedule_id: i64,
    pub sequence_id: i64,
    pub action: String,
    pub payload: String,
    pub time_offset: i64,
    pub continue_on_failure: bool,
    pub created: Option<DateTime<Utc>>,
}

pub struct SourceAllocation {
    pub id: i64,
    pub node_id: i64,
    pub ip: String,
    pub ip_alias: Option<String>,
    pub port: i64,
    pub server_id: Option<i64>,
    pub notes: Option<String>,
    pub created: Option<DateTime<Utc>>,
    pub locked: bool, // plugged.host: Pelican's `is_locked` (docs/features/allocations.md)
}

pub struct SourceData {
    pub settings: SourceSettings,
    pub backup_configs: Vec<SourceBackupConfig>,
    pub users: Vec<SourceUser>,
    pub ssh_keys: Vec<SourceSshKey>,
    pub locations: Vec<SourceLocation>,
    pub nodes: Vec<SourceNode>,
    pub nests: Vec<SourceNest>,
    pub eggs: Vec<SourceEgg>,
    pub egg_variables: Vec<SourceEggVariable>,
    pub database_hosts: Vec<SourceDatabaseHost>,
    pub servers: Vec<SourceServer>,
    pub databases: Vec<SourceDatabase>,
    pub server_variables: Vec<SourceServerVariable>,
    pub backups: Vec<SourceBackup>,
    pub subusers: Vec<SourceSubuser>,
    pub mounts: Vec<SourceMount>,
    pub egg_mounts: Vec<SourceMountLink>,
    pub node_mounts: Vec<SourceMountLink>,
    pub server_mounts: Vec<SourceMountLink>,
    pub schedules: Vec<SourceSchedule>,
    pub tasks: Vec<SourceTask>,
    pub allocations: Vec<SourceAllocation>,
    pub extras: super::extras::SourceExtras, // plugged.host: plugin data, icons, devboxes
}

/// Keys that already exist in the target database, loaded when importing with `--force`.
#[derive(Default)]
pub struct TargetSnapshot {
    pub user_uuids: Vec<uuid::Uuid>,
    pub usernames: Vec<String>,
    pub emails: Vec<String>,
    pub user_external_ids: Vec<String>,
    pub location_names: Vec<String>,
    pub node_uuids: Vec<uuid::Uuid>,
    pub node_names: Vec<String>,
    pub node_token_ids: Vec<String>,
    pub nest_uuids: Vec<uuid::Uuid>,
    pub nest_names: Vec<String>,
    pub egg_uuids: Vec<uuid::Uuid>,
    pub database_host_names: Vec<String>,
    pub server_uuids: Vec<uuid::Uuid>,
    pub server_uuid_shorts: Vec<i32>,
    pub backup_uuids: Vec<uuid::Uuid>,
    pub backup_configuration_names: Vec<String>,
    pub mount_uuids: Vec<uuid::Uuid>,
    pub mount_names: Vec<String>,
    pub mount_paths: Vec<(String, String)>,
}

pub struct PlanBackupConfig {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub disk: BackupDisk,
    pub s3: Option<shared::models::backup_configuration::BackupConfigsS3>,
}

pub struct PlanUser {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub external_id: Option<String>,
    pub username: String,
    pub email: String,
    pub name_first: Option<String>,
    pub name_last: Option<String>,
    pub password: Option<String>,
    pub admin: bool,
    pub totp_enabled: bool,
    pub totp_secret: Option<CompactString>,
    pub language: String,
    pub created: DateTime<Utc>,
}

pub struct PlanSshKey {
    pub source_id: i64,
    pub user_uuid: uuid::Uuid,
    pub name: String,
    pub fingerprint: String,
    pub public_key: Vec<u8>,
    pub created: DateTime<Utc>,
}

pub struct PlanLocation {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub backup_configuration_uuid: Option<uuid::Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub created: DateTime<Utc>,
    pub placement: super::mapping::LocationPlacement, // plugged.host: written by extras.rs
}

pub struct PlanNode {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub location_uuid: uuid::Uuid,
    pub backup_configuration_uuid: Option<uuid::Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub deployment_enabled: bool,
    pub maintenance_enabled: bool,
    pub url: String,
    pub public_url: Option<String>,
    pub sftp_host: Option<String>,
    pub sftp_port: i32,
    pub memory: i64,
    pub disk: i64,
    pub token_id: String,
    pub token: CompactString,
    pub created: DateTime<Utc>,
    // plugged.host: written by extras.rs
    pub role: CompactString,
    pub tags: Vec<String>,
    pub srv_target: Option<String>,
}

pub struct PlanNest {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub author: String,
    pub name: String,
    pub description: Option<String>,
    pub created: DateTime<Utc>,
}

pub struct PlanEgg {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub nest_uuid: uuid::Uuid,
    pub author: String,
    pub name: String,
    pub description: Option<String>,
    pub features: Vec<CompactString>,
    pub docker_images: IndexMap<CompactString, CompactString>,
    pub file_denylist: Vec<CompactString>,
    pub config_files: Vec<ProcessConfigurationFile>,
    pub config_startup: NestEggConfigStartup,
    pub config_stop: NestEggConfigStop,
    pub config_script: NestEggConfigScript,
    pub startup_commands: IndexMap<CompactString, CompactString>,
    pub force_outgoing_ip: bool,
    pub created: DateTime<Utc>,
}

pub struct PlanEggVariable {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub egg_uuid: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub order: i16,
    pub env_variable: String,
    pub default_value: Option<String>,
    pub user_viewable: bool,
    pub user_editable: bool,
    pub rules: Vec<CompactString>,
    pub created: DateTime<Utc>,
}

pub struct PlanDatabaseHost {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub name: String,
    pub host: CompactString,
    pub port: u16,
    pub username: CompactString,
    pub password: CompactString,
    pub node_uuids: Vec<uuid::Uuid>,
    pub location_uuids: Vec<uuid::Uuid>,
    pub created: DateTime<Utc>,
}

pub struct PlanServer {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub uuid_short: i32,
    pub external_id: Option<String>,
    pub node_uuid: uuid::Uuid,
    pub owner_uuid: uuid::Uuid,
    pub egg_uuid: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: Option<ServerStatus>,
    pub suspended: bool,
    pub memory: i64,
    pub swap: i64,
    pub disk: i64,
    pub io_weight: Option<i16>,
    pub cpu: i32,
    pub pinned_cpus: Vec<i16>,
    pub startup: String,
    pub image: String,
    pub labels: IndexMap<CompactString, CompactString>,
    pub allocation_limit: i32,
    pub database_limit: i32,
    pub backup_limit: i32,
    pub schedule_limit: i32,
    pub created: DateTime<Utc>,
}

pub struct PlanDatabase {
    pub source_id: i64,
    pub server_uuid: uuid::Uuid,
    pub database_host_uuid: uuid::Uuid,
    pub name: String,
    pub username: String,
    pub password: CompactString,
    pub created: DateTime<Utc>,
}

pub struct PlanServerVariable {
    pub source_id: i64,
    pub server_uuid: uuid::Uuid,
    pub variable_uuid: uuid::Uuid,
    pub value: String,
    pub created: DateTime<Utc>,
}

pub struct PlanBackup {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub server_uuid: uuid::Uuid,
    pub node_uuid: uuid::Uuid,
    pub backup_configuration_uuid: Option<uuid::Uuid>,
    pub name: String,
    pub successful: bool,
    pub locked: bool,
    pub ignored_files: Vec<String>,
    pub disk: BackupDisk,
    pub checksum: Option<String>,
    pub bytes: i64,
    pub upload_id: Option<String>,
    pub completed: Option<DateTime<Utc>>,
    pub deleted: Option<DateTime<Utc>>,
    pub created: DateTime<Utc>,
}

pub struct PlanSubuser {
    pub source_id: i64,
    pub server_uuid: uuid::Uuid,
    pub user_uuid: uuid::Uuid,
    pub permissions: Vec<CompactString>,
    pub created: DateTime<Utc>,
}

pub struct PlanMount {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub target: String,
    pub read_only: bool,
    pub user_mountable: bool,
}

pub struct PlanSchedule {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub server_uuid: uuid::Uuid,
    pub name: String,
    pub enabled: bool,
    pub triggers: Vec<wings_api::ScheduleTrigger>,
    pub condition: wings_api::ScheduleCondition,
    pub last_run: Option<DateTime<Utc>>,
    pub created: DateTime<Utc>,
}

pub struct PlanScheduleStep {
    pub source_id: i64,
    pub schedule_uuid: uuid::Uuid,
    pub action: wings_api::ScheduleActionInner,
    pub order: i16,
    pub created: DateTime<Utc>,
}

pub struct PlanAllocation {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub node_uuid: uuid::Uuid,
    pub ip: sqlx::types::ipnetwork::IpNetwork,
    pub ip_alias: Option<String>,
    pub port: i32,
    pub created: DateTime<Utc>,
}

pub struct PlanServerAllocation {
    pub source_id: i64,
    pub uuid: uuid::Uuid,
    pub server_uuid: uuid::Uuid,
    pub allocation_uuid: uuid::Uuid,
    pub notes: Option<String>,
    pub primary: bool,
    pub created: DateTime<Utc>,
    pub locked: bool, // plugged.host
}

#[derive(Default)]
pub struct Plan {
    pub backup_configs: Vec<PlanBackupConfig>,
    pub users: Vec<PlanUser>,
    pub ssh_keys: Vec<PlanSshKey>,
    pub locations: Vec<PlanLocation>,
    pub nodes: Vec<PlanNode>,
    pub nests: Vec<PlanNest>,
    pub eggs: Vec<PlanEgg>,
    pub egg_variables: Vec<PlanEggVariable>,
    pub database_hosts: Vec<PlanDatabaseHost>,
    pub servers: Vec<PlanServer>,
    pub databases: Vec<PlanDatabase>,
    pub server_variables: Vec<PlanServerVariable>,
    pub backups: Vec<PlanBackup>,
    pub subusers: Vec<PlanSubuser>,
    pub mounts: Vec<PlanMount>,
    pub egg_mounts: Vec<(uuid::Uuid, uuid::Uuid)>,
    pub node_mounts: Vec<(uuid::Uuid, uuid::Uuid)>,
    pub server_mounts: Vec<(uuid::Uuid, uuid::Uuid)>,
    pub schedules: Vec<PlanSchedule>,
    pub schedule_steps: Vec<PlanScheduleStep>,
    pub allocations: Vec<PlanAllocation>,
    pub server_allocations: Vec<PlanServerAllocation>,
    pub extras: super::extras::PlanExtras, // plugged.host: plugin data, icons, noprofit.space links
}

impl Plan {
    pub fn counts(&self) -> Vec<(&'static str, usize)> {
        // plugged.host: and the extras' below
        let mut counts = vec![
            ("users", self.users.len()),
            ("ssh keys", self.ssh_keys.len()),
            ("locations", self.locations.len()),
            ("nodes", self.nodes.len()),
            ("nests", self.nests.len()),
            ("eggs", self.eggs.len()),
            ("egg variables", self.egg_variables.len()),
            ("database hosts", self.database_hosts.len()),
            ("servers", self.servers.len()),
            ("server databases", self.databases.len()),
            ("server variables", self.server_variables.len()),
            ("backups", self.backups.len()),
            ("subusers", self.subusers.len()),
            ("mounts", self.mounts.len()),
            ("schedules", self.schedules.len()),
            ("schedule steps", self.schedule_steps.len()),
            ("allocations", self.allocations.len()),
        ];
        counts.extend(self.extras.counts()); // plugged.host

        counts
    }
}

pub type IdMap = HashMap<i64, uuid::Uuid>;
