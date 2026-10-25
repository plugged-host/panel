import { sql } from 'drizzle-orm';
import {
  bigint,
  boolean,
  char,
  check,
  date,
  customType,
  foreignKey,
  index,
  inet,
  integer,
  json,
  jsonb,
  PgColumn,
  pgEnum,
  pgSequence,
  pgTable,
  primaryKey,
  real,
  smallint,
  text,
  timestamp,
  uniqueIndex,
  uuid,
  varchar,
} from 'drizzle-orm/pg-core';

export const UTF8_MAX_SCALAR_SIZE = 4;

export const bytea = customType<{
  data: string;
  notNull: false;
  default: false;
}>({
  dataType() {
    return 'bytea';
  },

  toDriver(val: string) {
    let newVal = val;
    if (val.startsWith('0x')) {
      newVal = val.slice(2);
    }

    return Buffer.from(newVal, 'hex');
  },

  fromDriver(val: unknown) {
    return (val as Buffer).toString('hex');
  },
});

export const databaseTypeEnum = pgEnum('database_type', ['MYSQL', 'POSTGRES', 'MONGODB']);
export const databaseAgentTypeEnum = pgEnum('database_agent_type', ['POSTGRES', 'MARIADB', 'MONGODB', 'REDIS']);
export const serverStatusEnum = pgEnum('server_status', [
  'INSTALLING',
  'INSTALL_FAILED',
  'RESTORING_BACKUP',
  'BACKUP_RESTORE_FAILED',
]);
export const serverAutoStartBehaviorEnum = pgEnum('server_auto_start_behavior', ['ALWAYS', 'UNLESS_STOPPED', 'NEVER']);
export const backupDiskEnum = pgEnum('backup_disk', [
  'LOCAL',
  'S3',
  'DDUP_BAK',
  'BTRFS',
  'ZFS',
  'RESTIC',
  'PROXMOX_BACKUP_SERVER',
  'KOPIA',
]);
export const serverBackupKindEnum = pgEnum('server_backup_kind', ['SERVER', 'DATABASE_INSTANCE']);
export const serverDatabaseInstanceStatusEnum = pgEnum('server_database_instance_status', ['RESTORING_BACKUP']);
export const announcementTypeEnum = pgEnum('announcement_type', ['INFO', 'SUCCESS', 'WARNING', 'ERROR']);
export const tunnelProtocolEnum = pgEnum('tunnel_protocol', ['TCP', 'UDP']);

// Tables
export const settingsTable = pgTable('settings', {
  key: varchar({ length: 512 }).primaryKey().notNull(),
  value: text().notNull(),
});

export const versionHistoryTable = pgTable(
  'version_history',
  {
    extension: varchar({ length: 255 }).notNull(),
    version: varchar({ length: 255 }).notNull(),
    installed: timestamp().defaultNow().notNull(),
  },
  (cols) => [primaryKey({ name: 'version_history_extension_version_pk', columns: [cols.extension, cols.version] })],
);

export const announcementsTable = pgTable(
  'announcements',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    type: announcementTypeEnum().notNull(),
    enabled: boolean().default(false).notNull(),
    enabled_start: timestamp(),
    enabled_end: timestamp(),
    dismissible: boolean().default(false).notNull(),
    dismissible_end: timestamp(),
    title: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    title_translations: jsonb().default({}).notNull(),
    content: text().notNull(),
    content_translations: jsonb().default({}).notNull(),
    locations: uuid().array().default([]).notNull(),
    nodes: uuid().array().default([]).notNull(),
    backup_configurations: uuid().array().default([]).notNull(),
    eggs: uuid().array().default([]).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('announcements_enabled_idx').on(cols.enabled),
    index('announcements_enabled_start_idx').on(cols.enabled_start),
    index('announcements_enabled_end_idx').on(cols.enabled_end),
    index('announcements_locations_idx').using('gin', cols.locations),
    index('announcements_nodes_idx').using('gin', cols.nodes),
    index('announcements_backup_configurations_idx').using('gin', cols.backup_configurations),
    index('announcements_eggs_idx').using('gin', cols.eggs),
  ],
);

export const emailTemplatesTable = pgTable('email_templates', {
  identifier: varchar({ length: 255 }).primaryKey().notNull(),
  enabled: boolean().default(true).notNull(),
  subject: varchar({ length: 255 }),
  content: text(),
});

export const emailVariablesTable = pgTable(
  'email_variables',
  {
    template_identifier: varchar({ length: 255 }).notNull(),
    name: varchar({ length: 64 }).notNull(),
    value: text(),
    value_translations: jsonb().default({}).notNull(),
  },
  (cols) => [
    primaryKey({
      name: 'email_variables_template_identifier_name_pk',
      columns: [cols.template_identifier, cols.name],
    }),
  ],
);

export const usersTable = pgTable(
  'users',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    role_uuid: uuid().references(() => rolesTable.uuid, { onDelete: 'set null' }),
    external_id: varchar({ length: 255 }),
    avatar: varchar({ length: 255 }),
    username: varchar({ length: 15 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    email: varchar({ length: 255 }).notNull(),
    name_first: varchar({ length: 255 }),
    name_last: varchar({ length: 255 }),
    password: text(),
    admin: boolean().default(false).notNull(),
    frozen: boolean().default(false).notNull(),
    suspended: boolean().default(false).notNull(),
    totp_enabled: boolean().default(false).notNull(),
    totp_last_used: timestamp(),
    totp_last_step: bigint({ mode: 'number' }),
    totp_secret: bytea(),
    email_two_factor_enabled: boolean().default(false).notNull(),
    email_verified: boolean().default(false).notNull(),
    password_login_disabled: boolean().default(false).notNull(),
    language: varchar({ length: 15 }).default('en').notNull(),
    // plugged.host: the economy (shared/src/economy.rs): credits and the resources they bought
    // (without noprofit.space), tokens and the day of the last daily token
    credits: bigint({ mode: 'number' }).default(0).notNull(),
    extra_cpu: integer().default(0).notNull(),
    extra_memory: integer().default(0).notNull(),
    extra_disk: integer().default(0).notNull(),
    tokens: bigint({ mode: 'number' }).default(0).notNull(),
    tokens_earned: bigint({ mode: 'number' }).default(0).notNull(),
    tokens_daily_granted: date({ mode: 'string' }),
    // plugged.host: the memberships' burst cores, kept by the economy
    burst_cores: smallint().default(0).notNull(),
    // plugged.host: paid panel access (shared/src/panel_access.rs): the moment the current month
    // runs out, empty for anyone who has never paid
    panel_access_until: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('users_role_uuid_idx').on(cols.role_uuid),
    index('users_created_idx').on(cols.created),
    uniqueIndex('users_external_id_idx').on(cols.external_id),
    uniqueIndex('users_username_idx').on(sql`lower(${cols.username})`),
    uniqueIndex('users_email_idx').on(sql`lower(${cols.email})`),
  ],
);

export const adminActivitiesTable = pgTable(
  'admin_activities',
  {
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    impersonator_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    api_key_uuid: uuid().references(() => userApiKeysTable.uuid, { onDelete: 'set null' }),
    event: varchar({ length: 255 }).notNull(),
    ip: inet(),
    data: jsonb().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('admin_activities_impersonator_uuid_idx').on(cols.impersonator_uuid),
    index('admin_activities_api_key_uuid_idx').on(cols.api_key_uuid),
    index('admin_activities_created_idx').on(cols.created),
    index('admin_activities_user_uuid_created_idx').on(cols.user_uuid, cols.created),
  ],
);

export const userActivitiesTable = pgTable(
  'user_activities',
  {
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    impersonator_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    api_key_uuid: uuid().references(() => userApiKeysTable.uuid, { onDelete: 'set null' }),
    event: varchar({ length: 255 }).notNull(),
    ip: inet(),
    data: jsonb().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('user_activities_impersonator_uuid_idx').on(cols.impersonator_uuid),
    index('user_activities_api_key_uuid_idx').on(cols.api_key_uuid),
    index('user_activities_created_idx').on(cols.created),
    index('user_activities_user_uuid_created_idx').on(cols.user_uuid, cols.created),
  ],
);

export const userSessionsTable = pgTable(
  'user_sessions',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    key_id: char({ length: 16 }).notNull(),
    key: text().notNull(),
    ip: inet().notNull(),
    user_agent: varchar({ length: 255 }).notNull(),
    last_used: timestamp().defaultNow().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('user_sessions_user_uuid_idx').on(cols.user_uuid),
    uniqueIndex('user_sessions_key_idx').on(cols.key),
    index('user_sessions_key_id_idx').on(cols.key_id),
    index('user_sessions_last_used_idx').on(cols.last_used),
  ],
);

export const userRecoveryCodesTable = pgTable(
  'user_recovery_codes',
  {
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    code: char({ length: 10 }).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'user_recovery_codes_user_uuid_code_idx', columns: [cols.user_uuid, cols.code] }),
    index('user_recovery_codes_user_uuid_idx').on(cols.user_uuid),
  ],
);

export const userSettingsTable = pgTable('user_settings', {
  user_uuid: uuid()
    .references(() => usersTable.uuid, { onDelete: 'cascade' })
    .primaryKey()
    .notNull(),
  settings: jsonb().notNull(),
});

export const userPasswordResetsTable = pgTable(
  'user_password_resets',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    token_start: char({ length: 16 }).notNull(),
    token: text().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('user_password_resets_user_uuid_idx').on(cols.user_uuid),
    uniqueIndex('user_password_resets_token_idx').on(cols.token),
    index('user_password_resets_token_start_idx').on(cols.token_start),
  ],
);

export const userEmailVerificationsTable = pgTable(
  'user_email_verifications',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    email: varchar({ length: 255 }).notNull(),
    token_start: char({ length: 16 }).notNull(),
    token: text().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('user_email_verifications_user_uuid_idx').on(cols.user_uuid),
    uniqueIndex('user_email_verifications_token_idx').on(cols.token),
    index('user_email_verifications_token_start_idx').on(cols.token_start),
  ],
);

export const userTwoFactorCodesTable = pgTable(
  'user_two_factor_codes',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    code: text().notNull(),
    attempts: integer().default(0).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('user_two_factor_codes_user_uuid_idx').on(cols.user_uuid)],
);

export const userSecurityKeysTable = pgTable(
  'user_security_keys',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 31 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    credential_id: bytea().notNull(),
    passkey: jsonb(),
    registration: jsonb(),
    last_used: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('user_security_keys_user_uuid_idx').on(cols.user_uuid),
    uniqueIndex('user_security_keys_user_uuid_name_idx').on(cols.user_uuid, cols.name),
    uniqueIndex('user_security_keys_credential_id_idx').on(cols.credential_id),
    index('user_security_keys_created_idx').on(cols.created),
  ],
);

export const userSshKeysTable = pgTable(
  'user_ssh_keys',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 31 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    fingerprint: char({ length: 50 }).notNull(),
    public_key: bytea().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('user_ssh_keys_user_uuid_idx').on(cols.user_uuid),
    uniqueIndex('user_ssh_keys_user_uuid_name_idx').on(cols.user_uuid, cols.name),
    uniqueIndex('user_ssh_keys_user_uuid_fingerprint_idx').on(cols.user_uuid, cols.fingerprint),
  ],
);

export const userApiKeysTable = pgTable(
  'user_api_keys',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 31 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    key_start: char({ length: 16 }).notNull(),
    key: text().notNull(),
    allowed_ips: inet().array().default([]).notNull(),
    enabled: boolean().default(true).notNull(),
    user_permissions: varchar({ length: 64 }).array().notNull(),
    admin_permissions: varchar({ length: 64 }).array().notNull(),
    server_permissions: varchar({ length: 64 }).array().notNull(),
    last_used: timestamp(),
    expires: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('user_api_keys_user_uuid_idx').on(cols.user_uuid),
    uniqueIndex('user_api_keys_user_uuid_name_idx').on(cols.user_uuid, cols.name),
    uniqueIndex('user_api_keys_user_uuid_key_start_idx').on(cols.user_uuid, cols.key_start),
    uniqueIndex('user_api_keys_key_idx').on(cols.key),
    index('user_api_keys_expires_idx').on(cols.expires),
  ],
);

export const userOauthLinksTable = pgTable(
  'user_oauth_links',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    oauth_provider_uuid: uuid()
      .references(() => oauthProvidersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    identifier: varchar({ length: 255 }).notNull(),
    last_used: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('user_oauth_links_user_uuid_idx').on(cols.user_uuid),
    index('user_oauth_links_oauth_provider_uuid_idx').on(cols.oauth_provider_uuid),
    uniqueIndex('user_oauth_links_user_uuid_oauth_provider_uuid_idx').on(cols.user_uuid, cols.oauth_provider_uuid),
    uniqueIndex('user_oauth_links_oauth_provider_uuid_identifier_idx').on(cols.oauth_provider_uuid, cols.identifier),
  ],
);

// plugged.host: the noprofit.space account a user signs in with, and its latest snapshot
// (shared/src/noprofit.rs). `deleted` is set when noprofit.space no longer has the account.
export const userNoprofitAccountsTable = pgTable(
  'user_noprofit_accounts',
  {
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .primaryKey()
      .notNull(),
    account_id: bigint({ mode: 'number' }).notNull(),
    email: varchar({ length: 255 }).notNull(),
    username: varchar({ length: 255 }).notNull(),
    is_admin: boolean().default(false).notNull(),
    avatar: text(),
    credits: bigint({ mode: 'number' }).default(0).notNull(),
    plan: jsonb(),
    discord: jsonb(),
    deleted: timestamp(),
    synced: timestamp().defaultNow().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('user_noprofit_accounts_account_id_idx').on(cols.account_id)],
);

// plugged.host: the economy's ledgers, earn claims and memberships (shared/src/economy.rs,
// shared/src/earn.rs, docs/features/economy.md)
export const userCreditTransactionsTable = pgTable(
  'user_credit_transactions',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    amount: bigint({ mode: 'number' }).notNull(),
    balance_after: bigint({ mode: 'number' }).notNull(),
    type: varchar({ length: 64 }).notNull(),
    description: varchar({ length: 255 }).notNull(),
    data: jsonb().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('user_credit_transactions_user_uuid_created_idx').on(cols.user_uuid, cols.created)],
);

export const userTokenTransactionsTable = pgTable(
  'user_token_transactions',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    amount: bigint({ mode: 'number' }).notNull(),
    balance_after: bigint({ mode: 'number' }).notNull(),
    type: varchar({ length: 64 }).notNull(),
    description: varchar({ length: 255 }).notNull(),
    data: jsonb().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('user_token_transactions_user_uuid_created_idx').on(cols.user_uuid, cols.created)],
);

export const userTokenClaimsTable = pgTable(
  'user_token_claims',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    provider: varchar({ length: 32 }).notNull(),
    nonce: varchar({ length: 64 }).notNull(),
    token_hash: char({ length: 64 }),
    status: varchar({ length: 16 }).default('pending').notNull(),
    amount: integer().default(0).notNull(),
    reason: varchar({ length: 32 }),
    ip: inet(),
    created: timestamp().defaultNow().notNull(),
    completed: timestamp(),
  },
  (cols) => [
    uniqueIndex('user_token_claims_nonce_idx').on(cols.nonce),
    uniqueIndex('user_token_claims_provider_token_hash_idx').on(cols.provider, cols.token_hash),
    index('user_token_claims_user_uuid_provider_status_idx').on(cols.user_uuid, cols.provider, cols.status),
  ],
);

export const userMembershipsTable = pgTable(
  'user_memberships',
  {
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .primaryKey()
      .notNull(),
    plus_since: timestamp(),
    plusplus_until: timestamp(),
    plusplus_purchases: integer().default(0).notNull(),
    // a lapse's servers left the premium locations
    plusplus_settled: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('user_memberships_plusplus_until_idx').on(cols.plusplus_until)],
);

// plugged.host: the Discord server's members the panel knows of, the invites they brought and the
// boost levels paid (shared/src/discord, docs/features/discord.md)
export const discordMembersTable = pgTable(
  'discord_members',
  {
    discord_id: varchar({ length: 32 }).primaryKey().notNull(),
    // the account the Discord account was linked to at the last check
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    username: varchar({ length: 64 }),
    avatar: varchar({ length: 64 }),
    in_guild: boolean().default(false).notNull(),
    premium_since: timestamp(),
    boosts: smallint().default(0).notNull(),
    // clawed-back invite credits the balance could not cover, taken from the next invite rewards
    invite_debt: bigint({ mode: 'number' }).default(0).notNull(),
    checked: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('discord_members_user_uuid_idx').on(cols.user_uuid),
    index('discord_members_checked_idx').on(cols.checked),
  ],
);

export const discordInvitesTable = pgTable(
  'discord_invites',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    inviter_discord_id: varchar({ length: 32 }).notNull(),
    joined_discord_id: varchar({ length: 32 }).notNull(),
    invite_code: varchar({ length: 32 }),
    account_created: timestamp().notNull(),
    joined: timestamp().notNull(),
    // old enough and not the inviter themselves
    eligible: boolean().notNull(),
    matures: timestamp().notNull(),
    rewarded: timestamp(),
    credits: bigint({ mode: 'number' }).default(0).notNull(),
    departed: timestamp(),
    leave_count: integer().default(0).notNull(),
    rejoin_count: integer().default(0).notNull(),
    revoked: timestamp(),
    revoked_credits: bigint({ mode: 'number' }).default(0).notNull(),
    last_seen: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('discord_invites_joined_discord_id_idx').on(cols.joined_discord_id),
    index('discord_invites_inviter_discord_id_idx').on(cols.inviter_discord_id),
    index('discord_invites_rewarded_matures_idx').on(cols.rewarded, cols.matures),
    index('discord_invites_last_seen_idx').on(cols.last_seen),
  ],
);

export const discordBoostPayoutsTable = pgTable(
  'discord_boost_payouts',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    discord_id: varchar({ length: 32 }).notNull(),
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    level: smallint().notNull(),
    credits: bigint({ mode: 'number' }).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('discord_boost_payouts_discord_id_level_idx').on(cols.discord_id, cols.level),
    uniqueIndex('discord_boost_payouts_user_uuid_level_idx').on(cols.user_uuid, cols.level),
  ],
);

export const userCommandSnippetsTable = pgTable(
  'user_command_snippets',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 31 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    eggs: uuid().array().default([]).notNull(),
    command: text().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('command_snippets_user_uuid_idx').on(cols.user_uuid),
    index('command_snippets_eggs_idx').using('gin', cols.eggs),
    uniqueIndex('command_snippets_user_uuid_name_idx').on(cols.user_uuid, cols.name),
  ],
);

export const userServerGroupsTable = pgTable(
  'user_server_groups',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 31 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    order_: smallint().default(0).notNull(),
    server_order: uuid().array().default([]).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('server_groups_user_uuid_idx').on(cols.user_uuid)],
);

export const rolesTable = pgTable(
  'roles',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    require_two_factor: boolean().default(false).notNull(),
    admin_permissions: varchar({ length: 64 }).array().notNull(),
    server_permissions: varchar({ length: 64 }).array().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('roles_name_idx').on(cols.name)],
);

export const oauthProvidersTable = pgTable(
  'oauth_providers',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    icon: varchar({ length: 128 }),
    client_id: varchar({ length: 255 }).notNull(),
    client_secret: bytea(),
    auth_url: varchar({ length: 255 }).notNull(),
    token_url: varchar({ length: 255 }).notNull(),
    info_url: varchar({ length: 255 }).notNull(),
    info_urls: json().default({}).notNull(),
    scopes: varchar({ length: 64 }).array().notNull(),
    identifier_path: varchar({ length: 255 }).notNull(),
    email_path: varchar({ length: 255 }),
    username_path: varchar({ length: 255 }),
    name_first_path: varchar({ length: 255 }),
    name_last_path: varchar({ length: 255 }),
    avatar_url_template: varchar({ length: 255 }),
    avatar_overwrite: boolean().default(false).notNull(),
    enabled: boolean().default(false).notNull(),
    login_only: boolean().default(false).notNull(),
    login_bypass_two_factor: boolean().default(true).notNull(),
    link_viewable: boolean().default(false).notNull(),
    user_manageable: boolean().default(false).notNull(),
    basic_auth: boolean().default(false).notNull(),
    pkce: boolean().default(false).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('oauth_providers_name_idx').on(cols.name)],
);

export const oauthProviderMappingsTable = pgTable(
  'oauth_provider_mappings',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    oauth_provider_uuid: uuid()
      .references(() => oauthProvidersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    matcher: jsonb().notNull(),
    mapping: jsonb().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('oauth_provider_mappings_oauth_provider_uuid_idx').on(cols.oauth_provider_uuid)],
);

export const mountsTable = pgTable(
  'mounts',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    source: varchar({ length: 255 }).notNull(),
    target: varchar({ length: 255 }).notNull(),
    read_only: boolean().default(false).notNull(),
    user_mountable: boolean().default(false).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('mounts_name_idx').on(cols.name),
    uniqueIndex('mounts_source_target_idx').on(cols.source, cols.target),
  ],
);

export const devicesTable = pgTable(
  'devices',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    source: varchar({ length: 255 }).notNull(),
    target: varchar({ length: 255 }).notNull(),
    permissions: varchar({ length: 255 }).default('rwm').notNull(),
    user_attachable: boolean().default(false).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('devices_name_idx').on(cols.name),
    uniqueIndex('devices_source_target_idx').on(cols.source, cols.target),
  ],
);

export const backupConfigurationsTable = pgTable(
  'backup_configurations',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    maintenance_enabled: boolean().default(false).notNull(),
    shared: boolean().default(false).notNull(),
    description: text(),
    backup_disk: backupDiskEnum().default('LOCAL').notNull(),
    backup_configs: jsonb().default({}).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('backup_configurations_name_idx').on(cols.name)],
);

export const systemBackupPoliciesTable = pgTable(
  'system_backup_policies',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    backup_configuration_uuid: uuid().references(() => backupConfigurationsTable.uuid, { onDelete: 'set null' }),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    enabled: boolean().default(true).notNull(),
    kind: serverBackupKindEnum().default('SERVER').notNull(),
    cron: varchar({ length: 255 }).notNull(),
    retention: jsonb().default({ count: 0, days: 0, daily: 0, weekly: 0, monthly: 0, yearly: 0 }).notNull(),
    parallelism: integer().default(2).notNull(),
    triggered: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('system_backup_policies_backup_configuration_uuid_idx').on(cols.backup_configuration_uuid),
    uniqueIndex('system_backup_policies_name_idx').on(cols.name),
  ],
);

export const systemBackupPolicyNodesTable = pgTable(
  'system_backup_policy_nodes',
  {
    system_backup_policy_uuid: uuid()
      .references(() => systemBackupPoliciesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({
      name: 'system_backup_policy_nodes_pk',
      columns: [cols.system_backup_policy_uuid, cols.node_uuid],
    }),
    index('system_backup_policy_nodes_system_backup_policy_uuid_idx').on(cols.system_backup_policy_uuid),
    index('system_backup_policy_nodes_node_uuid_idx').on(cols.node_uuid),
  ],
);

export const systemBackupPolicyDatabaseAgentHostsTable = pgTable(
  'system_backup_policy_database_agent_hosts',
  {
    system_backup_policy_uuid: uuid()
      .references(() => systemBackupPoliciesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    database_agent_host_uuid: uuid()
      .references(() => databaseAgentHostsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({
      name: 'system_backup_policy_database_agent_hosts_pk',
      columns: [cols.system_backup_policy_uuid, cols.database_agent_host_uuid],
    }),
    index('system_backup_policy_database_agent_hosts_system_backup_policy_uuid_idx').on(cols.system_backup_policy_uuid),
    index('system_backup_policy_database_agent_hosts_database_agent_host_uuid_idx').on(cols.database_agent_host_uuid),
  ],
);

export const systemBackupPolicyLocationsTable = pgTable(
  'system_backup_policy_locations',
  {
    system_backup_policy_uuid: uuid()
      .references(() => systemBackupPoliciesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    location_uuid: uuid()
      .references(() => locationsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({
      name: 'system_backup_policy_locations_pk',
      columns: [cols.system_backup_policy_uuid, cols.location_uuid],
    }),
    index('system_backup_policy_locations_system_backup_policy_uuid_idx').on(cols.system_backup_policy_uuid),
    index('system_backup_policy_locations_location_uuid_idx').on(cols.location_uuid),
  ],
);

export const systemBackupPolicyServersTable = pgTable(
  'system_backup_policy_servers',
  {
    system_backup_policy_uuid: uuid()
      .references(() => systemBackupPoliciesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({
      name: 'system_backup_policy_servers_pk',
      columns: [cols.system_backup_policy_uuid, cols.server_uuid],
    }),
    index('system_backup_policy_servers_system_backup_policy_uuid_idx').on(cols.system_backup_policy_uuid),
    index('system_backup_policy_servers_server_uuid_idx').on(cols.server_uuid),
  ],
);

export const locationsTable = pgTable(
  'locations',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    backup_configuration_uuid: uuid().references(() => backupConfigurationsTable.uuid, { onDelete: 'set null' }),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    flag: char({ length: 2 }),
    // plugged.host: a label shown with the location, its place in lists, and whether users see it
    tier: varchar({ length: 32 }),
    sort: integer().default(0).notNull(),
    public: boolean().default(true).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('locations_backup_configuration_uuid_idx').on(cols.backup_configuration_uuid),
    uniqueIndex('locations_name_idx').on(cols.name),
  ],
);

// plugged.host: the resource presets offered when a player creates a server (Admin › Plans,
// docs/features/create-server.md). CPU in percent of one thread, memory and disk in MiB.
export const plansTable = pgTable(
  'plans',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    cpu: integer().notNull(),
    memory: bigint({ mode: 'number' }).notNull(),
    disk: bigint({ mode: 'number' }).notNull(),
    sort: integer().default(0).notNull(),
    public: boolean().default(true).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('plans_name_idx').on(cols.name)],
);

// plugged.host: sponsored banners shown in the ad slots (shared/src/models/ad_banner.rs)
export const adBannersTable = pgTable(
  'ad_banners',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    advertiser: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    link: varchar({ length: 1024 }).notNull(),
    placement: varchar({ length: 16 }).notNull(),
    image: varchar({ length: 255 }),
    width: integer().default(0).notNull(),
    height: integer().default(0).notNull(),
    starts: timestamp(),
    ends: timestamp(),
    active: boolean().default(true).notNull(),
    clicks: bigint({ mode: 'number' }).default(0).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('ad_banners_created_idx').on(cols.created)],
);

export const locationDatabaseHostsTable = pgTable(
  'location_database_hosts',
  {
    location_uuid: uuid()
      .references(() => locationsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    database_host_uuid: uuid()
      .references(() => databaseHostsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'location_database_hosts_pk', columns: [cols.location_uuid, cols.database_host_uuid] }),
    index('location_database_hosts_location_uuid_idx').on(cols.location_uuid),
    index('location_database_hosts_database_host_uuid_idx').on(cols.database_host_uuid),
  ],
);

export const nodesTable = pgTable(
  'nodes',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    location_uuid: uuid()
      .references(() => locationsTable.uuid)
      .notNull(),
    backup_configuration_uuid: uuid().references(() => backupConfigurationsTable.uuid, { onDelete: 'set null' }),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    deployment_enabled: boolean().default(false).notNull(),
    maintenance_enabled: boolean().default(false).notNull(),
    description: text(),
    public_url: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }),
    url: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    sftp_host: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }),
    sftp_port: integer().notNull(),
    memory: bigint({ mode: 'number' }).notNull(),
    disk: bigint({ mode: 'number' }).notNull(),
    token_id: char({ length: 16 }).notNull(),
    token: bytea().notNull(),
    // plugged.host: what the node is for, free labels, the host name SRV records point at, and the
    // automatic pause of deployment while it is overloaded (docs/features/node-telemetry.md)
    role: varchar({ length: 32 }).default('game').notNull(),
    tags: varchar({ length: 64 }).array().default([]).notNull(),
    srv_target: varchar({ length: 255 }),
    deployment_paused: boolean().default(false).notNull(),
    // plugged.host: why the panel paused it (memory, cpu, unreachable); null when an admin did
    deployment_paused_reason: varchar({ length: 32 }),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('nodes_location_uuid_idx').on(cols.location_uuid),
    index('nodes_backup_configuration_uuid_idx').on(cols.backup_configuration_uuid),
    uniqueIndex('nodes_name_idx').on(cols.name),
    uniqueIndex('nodes_token_id_idx').on(cols.token_id),
    uniqueIndex('nodes_token_idx').on(cols.token),
  ],
);

// plugged.host: the panel's view of each node, sampled every minute (shared/src/node_telemetry.rs).
// One row per node with the newest sample and the counters derived from it.
export const nodeTelemetryTable = pgTable('node_telemetry', {
  node_uuid: uuid()
    .references(() => nodesTable.uuid, { onDelete: 'cascade' })
    .primaryKey()
    .notNull(),
  reachable: boolean().notNull(),
  refused: boolean().default(false).notNull(),
  response_time_ms: integer(),
  cpu_percent: real(),
  cpu_threads: integer(),
  memory_used: bigint({ mode: 'number' }),
  memory_total: bigint({ mode: 'number' }),
  disk_used: bigint({ mode: 'number' }),
  disk_total: bigint({ mode: 'number' }),
  load1: real(),
  servers_running: integer(),
  pressure: real(),
  consecutive_failures: integer().default(0).notNull(),
  consecutive_outages: integer().default(0).notNull(),
  error: text(),
  last_reachable: timestamp(),
  sampled: timestamp().defaultNow().notNull(),
});

// plugged.host: every sample, kept for a week (the node's Statistics tab, the status page).
export const nodeTelemetrySamplesTable = pgTable(
  'node_telemetry_samples',
  {
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    reachable: boolean().notNull(),
    refused: boolean().default(false).notNull(),
    response_time_ms: integer(),
    cpu_percent: real(),
    memory_used: bigint({ mode: 'number' }),
    memory_total: bigint({ mode: 'number' }),
    disk_used: bigint({ mode: 'number' }),
    disk_total: bigint({ mode: 'number' }),
    load1: real(),
    servers_running: integer(),
    sampled: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'node_telemetry_samples_pk', columns: [cols.node_uuid, cols.sampled] }),
    index('node_telemetry_samples_sampled_idx').on(cols.sampled),
  ],
);

// plugged.host: the public status page (shared/src/status.rs). One component per probed endpoint
// (`site`, `panel`) and per node; its uuid is the public key the status files use.
export const statusComponentsTable = pgTable(
  'status_components',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    kind: varchar({ length: 16 }).notNull(),
    node_uuid: uuid().references(() => nodesTable.uuid, { onDelete: 'cascade' }),
    label: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    published: boolean().default(true).notNull(),
    sort: integer().default(0).notNull(),
    status: varchar({ length: 32 }).default('unknown').notNull(),
    failure_count: integer().default(0).notNull(),
    healthy_count: integer().default(0).notNull(),
    pressure_count: integer().default(0).notNull(),
    last_metrics: jsonb(),
    last_reported: timestamp(),
    checked: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('status_components_node_uuid_idx').on(cols.node_uuid),
    uniqueIndex('status_components_endpoint_kind_idx').on(cols.kind).where(sql`node_uuid IS NULL`),
    index('status_components_published_sort_idx').on(cols.published, cols.sort),
  ],
);

// plugged.host: per component and day, how many checks there were and how many were operational.
export const statusDailyUptimeTable = pgTable(
  'status_daily_uptime',
  {
    component_uuid: uuid()
      .references(() => statusComponentsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    date: date({ mode: 'string' }).notNull(),
    total_checks: integer().default(0).notNull(),
    operational_checks: integer().default(0).notNull(),
    worst_status: varchar({ length: 32 }).default('unknown').notNull(),
  },
  (cols) => [
    primaryKey({ name: 'status_daily_uptime_pk', columns: [cols.component_uuid, cols.date] }),
    index('status_daily_uptime_date_idx').on(cols.date),
  ],
);

// plugged.host: incidents posted by admins. Empty `components` means every component.
export const statusIncidentsTable = pgTable(
  'status_incidents',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    title: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    impact: varchar({ length: 32 }).default('minor').notNull(),
    status: varchar({ length: 32 }).default('investigating').notNull(),
    components: uuid().array().default([]).notNull(),
    created_by: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    started: timestamp().defaultNow().notNull(),
    resolved: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('status_incidents_status_started_idx').on(cols.status, cols.started)],
);

export const statusIncidentUpdatesTable = pgTable(
  'status_incident_updates',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    incident_uuid: uuid()
      .references(() => statusIncidentsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    status: varchar({ length: 32 }).notNull(),
    message: text().notNull(),
    created_by: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    published: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('status_incident_updates_incident_uuid_published_idx').on(cols.incident_uuid, cols.published)],
);

// plugged.host: an admin forcing a component's status (no component: all of them) for a while.
export const statusOverridesTable = pgTable(
  'status_overrides',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    component_uuid: uuid().references(() => statusComponentsTable.uuid, { onDelete: 'cascade' }),
    status: varchar({ length: 32 }).notNull(),
    note: text(),
    starts: timestamp(),
    ends: timestamp(),
    created_by: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('status_overrides_component_uuid_idx').on(cols.component_uuid)],
);

export const nodeMountsTable = pgTable(
  'node_mounts',
  {
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    mount_uuid: uuid()
      .references(() => mountsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'node_mounts_pk', columns: [cols.node_uuid, cols.mount_uuid] }),
    index('node_mounts_node_uuid_idx').on(cols.node_uuid),
    index('node_mounts_mount_uuid_idx').on(cols.mount_uuid),
  ],
);

export const nodeDevicesTable = pgTable(
  'node_devices',
  {
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    device_uuid: uuid()
      .references(() => devicesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'node_devices_pk', columns: [cols.node_uuid, cols.device_uuid] }),
    index('node_devices_node_uuid_idx').on(cols.node_uuid),
    index('node_devices_device_uuid_idx').on(cols.device_uuid),
  ],
);

export const nodeDatabaseHostsTable = pgTable(
  'node_database_hosts',
  {
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    database_host_uuid: uuid()
      .references(() => databaseHostsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'node_database_hosts_pk', columns: [cols.node_uuid, cols.database_host_uuid] }),
    index('node_database_hosts_node_uuid_idx').on(cols.node_uuid),
    index('node_database_hosts_database_host_uuid_idx').on(cols.database_host_uuid),
  ],
);

export const nodeAllocationsTable = pgTable(
  'node_allocations',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    ip: inet().notNull(),
    ip_alias: varchar({ length: 255 }),
    port: integer().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('allocations_node_uuid_ip_inet_port_idx').on(cols.node_uuid, cols.ip, cols.port),
    uniqueIndex('allocations_node_uuid_ip_port_idx').on(cols.node_uuid, sql`host(${cols.ip})`, cols.port),
  ],
);

export const eggRepositoriesTable = pgTable(
  'egg_repositories',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    git_repository: text().notNull(),
    credentials: jsonb().default({ type: 'none' }).notNull(),
    default_nest_uuid: uuid().references(() => nestsTable.uuid, { onDelete: 'set null' }),
    auto_sync: boolean().default(true).notNull(),
    last_synced: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('egg_repositories_name_idx').on(cols.name),
    uniqueIndex('egg_repositories_git_repository_idx').on(cols.git_repository),
  ],
);

export const eggRepositoriesEggsTable = pgTable(
  'egg_repository_eggs',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    egg_repository_uuid: uuid()
      .references(() => eggRepositoriesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    path: text().notNull(),
    readme: text(),
    banner: text(),
    exported_egg: json().notNull(),
    hash: varchar({ length: 64 }),
    updated: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('egg_repository_eggs_egg_repository_uuid_idx').on(cols.egg_repository_uuid),
    uniqueIndex('egg_repository_eggs_egg_repository_uuid_path_idx').on(cols.egg_repository_uuid, cols.path),
  ],
);

export const eggConfigurationsTable = pgTable(
  'egg_configurations',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    order_: smallint().default(0).notNull(),
    matcher: jsonb().notNull(),
    config_allocations: jsonb(),
    config_startup: jsonb(),
    config_routes: jsonb(),
    config_features: jsonb(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('egg_configurations_name_idx').on(cols.name)],
);

export const nestsTable = pgTable(
  'nests',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    author: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('nests_name_idx').on(cols.name)],
);

export const nestEggsTable = pgTable(
  'nest_eggs',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    nest_uuid: uuid()
      .references(() => nestsTable.uuid)
      .notNull(),
    egg_repository_egg_uuid: uuid().references(() => eggRepositoriesEggsTable.uuid, { onDelete: 'set null' }),
    egg_repository_egg_hash: varchar({ length: 64 }),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    author: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    config_files: jsonb().notNull(),
    config_startup: jsonb().notNull(),
    config_stop: jsonb().notNull(),
    config_script: jsonb().notNull(),
    startup_commands: json().notNull(),
    force_outgoing_ip: boolean().default(false).notNull(),
    separate_port: boolean().default(false).notNull(),
    features: text().array().notNull(),
    docker_images: json().notNull(),
    file_denylist: text().array().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('eggs_nest_uuid_idx').on(cols.nest_uuid),
    index('eggs_egg_repository_egg_uuid_idx').on(cols.egg_repository_egg_uuid),
    uniqueIndex('eggs_nest_uuid_name_idx').on(cols.nest_uuid, cols.name),
  ],
);

export const nestEggMountsTable = pgTable(
  'nest_egg_mounts',
  {
    egg_uuid: uuid()
      .references(() => nestEggsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    mount_uuid: uuid()
      .references(() => mountsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'egg_mounts_pk', columns: [cols.egg_uuid, cols.mount_uuid] }),
    index('egg_mounts_egg_uuid_idx').on(cols.egg_uuid),
    index('egg_mounts_mount_uuid_idx').on(cols.mount_uuid),
  ],
);

export const nestEggDevicesTable = pgTable(
  'nest_egg_devices',
  {
    egg_uuid: uuid()
      .references(() => nestEggsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    device_uuid: uuid()
      .references(() => devicesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'egg_devices_pk', columns: [cols.egg_uuid, cols.device_uuid] }),
    index('egg_devices_egg_uuid_idx').on(cols.egg_uuid),
    index('egg_devices_device_uuid_idx').on(cols.device_uuid),
  ],
);

export const nestEggVariablesTable = pgTable(
  'nest_egg_variables',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    egg_uuid: uuid()
      .references(() => nestEggsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    name_translations: jsonb().default({}).notNull(),
    description: text(),
    description_translations: jsonb().default({}).notNull(),
    order_: smallint().default(0).notNull(),
    env_variable: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    default_value: text(),
    user_viewable: boolean().default(true).notNull(),
    user_editable: boolean().default(false).notNull(),
    secret: boolean().default(false).notNull(),
    rules: text().array().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('egg_variables_egg_uuid_idx').on(cols.egg_uuid),
    uniqueIndex('egg_variables_egg_uuid_env_variable_idx').on(cols.egg_uuid, cols.env_variable),
  ],
);

export const databaseHostsTable = pgTable(
  'database_hosts',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    deployment_enabled: boolean().default(false).notNull(),
    maintenance_enabled: boolean().default(false).notNull(),
    type: databaseTypeEnum().notNull(),
    public_host: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }),
    public_port: integer(),
    credentials: jsonb().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('database_hosts_name_idx').on(cols.name)],
);

// plugged.host: the domains server subdomains are made under, each a Cloudflare zone
// (shared/src/subdomains, docs/features/subdomains.md)
export const domainsTable = pgTable(
  'domains',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 253 }).notNull(),
    zone_id: varchar({ length: 64 }),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('domains_name_idx').on(cols.name)],
);

export const databaseAgentHostsTable = pgTable(
  'database_agent_hosts',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    deployment_enabled: boolean().default(false).notNull(),
    maintenance_enabled: boolean().default(false).notNull(),
    url: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    memory: bigint({ mode: 'number' }).notNull(),
    disk: bigint({ mode: 'number' }).notNull(),
    types: jsonb()
      .default({
        postgres: { enabled: true, public_host: null, public_port: null },
        mariadb: { enabled: true, public_host: null, public_port: null },
        mongodb: { enabled: true, public_host: null, public_port: null },
        redis: { enabled: true, public_host: null, public_port: null },
      })
      .notNull(),
    token: bytea().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('database_agent_hosts_name_idx').on(cols.name),
    uniqueIndex('database_agent_hosts_token_idx').on(cols.token),
  ],
);

export const locationDatabaseAgentHostsTable = pgTable(
  'location_database_agent_hosts',
  {
    location_uuid: uuid()
      .references(() => locationsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    database_agent_host_uuid: uuid()
      .references(() => databaseAgentHostsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({
      name: 'location_database_agent_hosts_pk',
      columns: [cols.location_uuid, cols.database_agent_host_uuid],
    }),
    index('location_database_agent_hosts_location_uuid_idx').on(cols.location_uuid),
    index('location_database_agent_hosts_database_agent_host_uuid_idx').on(cols.database_agent_host_uuid),
  ],
);

export const nodeDatabaseAgentHostsTable = pgTable(
  'node_database_agent_hosts',
  {
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    database_agent_host_uuid: uuid()
      .references(() => databaseAgentHostsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({
      name: 'node_database_agent_hosts_pk',
      columns: [cols.node_uuid, cols.database_agent_host_uuid],
    }),
    index('node_database_agent_hosts_node_uuid_idx').on(cols.node_uuid),
    index('node_database_agent_hosts_database_agent_host_uuid_idx').on(cols.database_agent_host_uuid),
  ],
);

export const databaseAgentTemplatesTable = pgTable(
  'database_agent_templates',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    type: databaseAgentTypeEnum().notNull(),
    deployment_enabled: boolean().default(false).notNull(),
    docker_images: json().notNull(),
    env: json().notNull(),
    image_uid: integer().notNull(),
    image_gid: integer().notNull(),
    cmd: text().array(),
    volumes: json().notNull(),
    socket_path: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    memory: bigint({ mode: 'number' }).notNull(),
    swap: bigint({ mode: 'number' }).notNull(),
    disk: bigint({ mode: 'number' }).notNull(),
    io_weight: smallint(),
    cpu: integer().notNull(),
    version: integer().default(1).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('database_agent_templates_name_idx').on(cols.name)],
);

export const serversTable = pgTable(
  'servers',
  {
    uuid: uuid().primaryKey().notNull(),
    uuid_short: integer().notNull(),
    external_id: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }),
    allocation_uuid: uuid().references((): PgColumn => serverAllocationsTable.uuid, { onDelete: 'set null' }),
    destination_allocation_uuid: uuid().references((): PgColumn => serverAllocationsTable.uuid, {
      onDelete: 'set null',
    }),
    node_uuid: uuid()
      .references(() => nodesTable.uuid)
      .notNull(),
    destination_node_uuid: uuid().references(() => nodesTable.uuid),
    owner_uuid: uuid()
      .references(() => usersTable.uuid)
      .notNull(),
    egg_uuid: uuid()
      .references(() => nestEggsTable.uuid)
      .notNull(),
    backup_configuration_uuid: uuid().references(() => backupConfigurationsTable.uuid, { onDelete: 'set null' }),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    description: text(),
    // plugged.host: the stored path of its icon (shared/src/models/server/icon.rs)
    icon: varchar({ length: 255 }),
    status: serverStatusEnum(),
    suspended: boolean().default(false).notNull(),
    // plugged.host: why, since when, by whom and until when it is suspended (shared/src/models/server/suspension.rs)
    suspension_kind: varchar({ length: 16 }),
    suspension_reason: text(),
    suspended_at: timestamp(),
    suspended_by: uuid().references((): PgColumn => usersTable.uuid, { onDelete: 'set null' }),
    suspension_delete_at: timestamp(),
    // plugged.host: the idle clock and where the server is in limbo (shared/src/limbo.rs)
    last_activity: timestamp().defaultNow().notNull(),
    limbo_state: varchar({ length: 16 }),
    memory: bigint({ mode: 'number' }).notNull(),
    memory_overhead: bigint({ mode: 'number' }).default(0).notNull(),
    swap: bigint({ mode: 'number' }).notNull(),
    disk: bigint({ mode: 'number' }).notNull(),
    bandwidth_upload: bigint({ mode: 'number' }).default(0).notNull(),
    bandwidth_download: bigint({ mode: 'number' }).default(0).notNull(),
    io_weight: smallint(),
    cpu: integer().notNull(),
    pinned_cpus: smallint().array().notNull(),
    startup: text().notNull(),
    image: varchar({ length: 255 }).notNull(),
    labels: json().default({}).notNull(),
    auto_kill: jsonb().default({ enabled: false, seconds: 30 }).notNull(),
    auto_start_behavior: serverAutoStartBehaviorEnum().default('UNLESS_STOPPED').notNull(),
    timezone: varchar({ length: 255 }),
    trash: jsonb(),
    hugepages_passthrough_enabled: boolean().default(false).notNull(),
    kvm_passthrough_enabled: boolean().default(false).notNull(),
    allocation_limit: integer().default(0).notNull(),
    database_limit: integer().default(0).notNull(),
    backup_limit: integer().default(0).notNull(),
    schedule_limit: integer().default(0).notNull(),
    subdomain_limit: integer(), // plugged.host: empty uses the subdomains settings' default
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('servers_external_id_idx').on(cols.external_id),
    index('servers_allocation_uuid_idx').on(cols.allocation_uuid),
    index('servers_node_uuid_idx').on(cols.node_uuid),
    index('servers_destination_node_uuid_idx').on(cols.destination_node_uuid),
    index('servers_owner_uuid_idx').on(cols.owner_uuid),
    index('servers_owner_uuid_created_idx').on(cols.owner_uuid, cols.created, cols.uuid),
    index('servers_egg_uuid_idx').on(cols.egg_uuid),
    index('servers_backup_configuration_uuid_idx').on(cols.backup_configuration_uuid),
    uniqueIndex('servers_uuid_short_idx').on(cols.uuid_short),
    index('servers_created_idx').on(cols.created),
    index('servers_suspension_delete_at_idx').on(cols.suspension_delete_at), // plugged.host
    index('servers_limbo_state_idx').on(cols.limbo_state), // plugged.host
  ],
);

// plugged.host: what the abuse checks found (shared/src/abuse.rs). One row per server (or node, for
// what runs on the node itself) and rule, updated on every hit rather than appended.
export const abuseFindingsTable = pgTable(
  'abuse_findings',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    fingerprint: varchar({ length: 160 }).notNull(),
    server_uuid: uuid().references(() => serversTable.uuid, { onDelete: 'cascade' }),
    node_uuid: uuid().references(() => nodesTable.uuid, { onDelete: 'cascade' }),
    rule: varchar({ length: 64 }).notNull(),
    category: varchar({ length: 32 }).notNull(),
    severity: varchar({ length: 16 }).notNull(),
    source: varchar({ length: 16 }).notNull(),
    status: varchar({ length: 16 }).default('open').notNull(),
    summary: varchar({ length: 500 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    evidence: jsonb().default({}).notNull(),
    hits: integer().default(0).notNull(),
    first_seen: timestamp().defaultNow().notNull(),
    last_seen: timestamp().defaultNow().notNull(),
    action: varchar({ length: 32 }),
    actioned: timestamp(),
    resolved_by: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    resolved: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('abuse_findings_fingerprint_idx').on(cols.fingerprint),
    index('abuse_findings_status_last_seen_idx').on(cols.status, cols.last_seen),
    index('abuse_findings_server_uuid_idx').on(cols.server_uuid),
  ],
);

// plugged.host: decreases of an account's allowance its servers give back (docs/features/allowance-cuts.md)
export const userAllowanceCutsTable = pgTable(
  'user_allowance_cuts',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    reason: varchar({ length: 64 }).notNull(),
    cpu: bigint({ mode: 'number' }).default(0).notNull(),
    memory: bigint({ mode: 'number' }).default(0).notNull(),
    disk: bigint({ mode: 'number' }).default(0).notNull(),
    result: jsonb(),
    created: timestamp().defaultNow().notNull(),
    settled: timestamp(),
  },
  (cols) => [index('user_allowance_cuts_settled_user_uuid_idx').on(cols.settled, cols.user_uuid)],
);

// plugged.host: deleted servers whose system backups stay downloadable for their owner a while
// (shared/src/deleted_servers.rs, docs/features/backups-page.md)
export const deletedServersTable = pgTable(
  'deleted_servers',
  {
    uuid: uuid().primaryKey().notNull(),
    owner_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    suspended: boolean().default(false).notNull(),
    backups: uuid().array().notNull(),
    deleted: timestamp().defaultNow().notNull(),
    hold_until: timestamp().notNull(),
    trimmed: timestamp(),
    purged: timestamp(),
  },
  (cols) => [
    index('deleted_servers_owner_uuid_idx').on(cols.owner_uuid),
    index('deleted_servers_purged_hold_until_idx').on(cols.purged, cols.hold_until),
  ],
);

// plugged.host: a node's servers restored on other nodes from their newest backup, when the node
// is down (shared/src/node_failover.rs, docs/features/failover.md)
export const nodeFailoversTable = pgTable(
  'node_failovers',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    mode: varchar({ length: 16 }).notNull(),
    status: varchar({ length: 16 }).notNull(),
    started_by: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    created: timestamp().defaultNow().notNull(),
    finished: timestamp(),
  },
  (cols) => [
    index('node_failovers_node_uuid_created_idx').on(cols.node_uuid, cols.created),
    index('node_failovers_status_idx').on(cols.status),
  ],
);

export const serverFailoversTable = pgTable(
  'server_failovers',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    node_failover_uuid: uuid()
      .references(() => nodeFailoversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    // no foreign key: the source node's copy is cleaned up even after the server is deleted
    server_uuid: uuid().notNull(),
    server_name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    source_node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    destination_node_uuid: uuid().references(() => nodesTable.uuid, { onDelete: 'set null' }),
    backup_uuid: uuid().references(() => serverBackupsTable.uuid, { onDelete: 'set null' }),
    status: varchar({ length: 16 }).notNull(),
    error: text(),
    created: timestamp().defaultNow().notNull(),
    updated: timestamp().defaultNow().notNull(),
    finished: timestamp(),
    source_cleaned: timestamp(),
  },
  (cols) => [
    index('server_failovers_node_failover_uuid_idx').on(cols.node_failover_uuid),
    index('server_failovers_status_idx').on(cols.status),
    index('server_failovers_server_uuid_idx').on(cols.server_uuid),
  ],
);

// plugged.host: where each user is in the welcome tour (shared/src/welcome_tour.rs,
// docs/features/welcome-tour.md)
export const userWelcomeToursTable = pgTable('user_welcome_tours', {
  user_uuid: uuid()
    .references(() => usersTable.uuid, { onDelete: 'cascade' })
    .primaryKey()
    .notNull(),
  version: integer().notNull(),
  step: integer().default(1).notNull(),
  server_step: integer().default(1).notNull(),
  completed: timestamp(),
  skipped: timestamp(),
  server_done: timestamp(),
  updated: timestamp().defaultNow().notNull(),
});

// plugged.host: moves of a server to another node, asked for by its owner or an admin's "Lighten
// load" (shared/src/server_transfers.rs, docs/features/transfers.md)
export const serverTransferRequestsTable = pgTable(
  'server_transfer_requests',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    origin: varchar({ length: 16 }).notNull(),
    location_uuid: uuid().references(() => locationsTable.uuid, { onDelete: 'set null' }),
    source_node_uuid: uuid().references(() => nodesTable.uuid, { onDelete: 'set null' }),
    destination_node_uuid: uuid().references(() => nodesTable.uuid, { onDelete: 'set null' }),
    tried_node_uuids: uuid().array().default([]).notNull(),
    attempts: integer().default(0).notNull(),
    status: varchar({ length: 16 }).notNull(),
    error: text(),
    restart: boolean().default(false).notNull(),
    variables: jsonb().default({}).notNull(),
    created: timestamp().defaultNow().notNull(),
    started: timestamp(),
    finished: timestamp(),
  },
  (cols) => [
    index('server_transfer_requests_server_uuid_created_idx').on(cols.server_uuid, cols.created),
    index('server_transfer_requests_status_idx').on(cols.status),
  ],
);

// plugged.host: starts waiting for room on a busy node, one row per server, reused
// (shared/src/start_queue.rs, docs/features/start-queue.md)
export const serverStartQueueTable = pgTable(
  'server_start_queue',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .primaryKey()
      .notNull(),
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    action: varchar({ length: 16 }).notNull(),
    status: varchar({ length: 16 }).notNull(),
    priority: smallint().default(0).notNull(),
    queued: timestamp().defaultNow().notNull(),
    last_seen: timestamp(),
    released: timestamp(),
    reason: varchar({ length: 16 }),
  },
  (cols) => [index('server_start_queue_node_uuid_status_idx').on(cols.node_uuid, cols.status)],
);

// plugged.host: a server's way through limbo: since when, its final backup, when it was parked, who
// paid for the restore and the last error (shared/src/limbo.rs)
export const serverLimboTable = pgTable('server_limbo', {
  server_uuid: uuid()
    .references(() => serversTable.uuid, { onDelete: 'cascade' })
    .primaryKey()
    .notNull(),
  since: timestamp().defaultNow().notNull(),
  backup_uuid: uuid().references(() => serverBackupsTable.uuid, { onDelete: 'set null' }),
  parked: timestamp(),
  paid: integer().default(0).notNull(),
  payer_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
  error: text(),
  failed: timestamp(),
  updated: timestamp().defaultNow().notNull(),
});

// plugged.host: one user's start waiting room for one server, and the pass it leaves
// (shared/src/waiting_room.rs)
export const serverWaitingRoomsTable = pgTable(
  'server_waiting_rooms',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    action: varchar({ length: 16 }).notNull(),
    opened: timestamp().defaultNow().notNull(),
    dispatched: boolean().default(false).notNull(),
    released: timestamp(),
    blocked: boolean(),
  },
  (cols) => [
    primaryKey({ name: 'server_waiting_rooms_server_uuid_user_uuid_pk', columns: [cols.server_uuid, cols.user_uuid] }),
  ],
);

// plugged.host: the setup a player's new Minecraft server starts with (docs/features/minecraft.md)
export const serverOnboardingsTable = pgTable('server_onboardings', {
  server_uuid: uuid()
    .references(() => serversTable.uuid, { onDelete: 'cascade' })
    .primaryKey()
    .notNull(),
  answers: jsonb(),
  completed: timestamp(),
  created: timestamp().defaultNow().notNull(),
});

export const serverAllocationsTable = pgTable(
  'server_allocations',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    allocation_uuid: uuid()
      .references(() => nodeAllocationsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    notes: text(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_allocations_server_uuid_idx').on(cols.server_uuid),
    uniqueIndex('server_allocations_allocation_uuid_idx').on(cols.allocation_uuid),
  ],
);

export const tunnelEpochSequence = pgSequence('tunnel_epoch');

export const nodeTunnelsTable = pgTable('node_tunnels', {
  node_uuid: uuid()
    .references(() => nodesTable.uuid, { onDelete: 'cascade' })
    .primaryKey()
    .notNull(),
  host: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
  port: integer().default(7100).notNull(),
  cert_sha256: bytea(),
  created: timestamp().defaultNow().notNull(),
});

export const serverTunnelsTable = pgTable(
  'server_tunnels',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .primaryKey()
      .notNull(),
    idx: integer().notNull(),
    name: varchar({ length: 63 }).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('server_tunnels_idx_idx').on(cols.idx),
    uniqueIndex('server_tunnels_server_uuid_name_idx').on(cols.server_uuid, cols.name),
  ],
);

export const serverTunnelPortsTable = pgTable(
  'server_tunnel_ports',
  {
    server_uuid: uuid()
      .references(() => serverTunnelsTable.server_uuid, { onDelete: 'cascade' })
      .notNull(),
    port: integer().notNull(),
    protocols: tunnelProtocolEnum().array().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'server_tunnel_ports_pk', columns: [cols.server_uuid, cols.port] }),
    index('server_tunnel_ports_server_uuid_idx').on(cols.server_uuid),
  ],
);

export const serverTunnelConnectionsTable = pgTable(
  'server_tunnel_connections',
  {
    src_server_uuid: uuid()
      .references(() => serverTunnelsTable.server_uuid, { onDelete: 'cascade' })
      .notNull(),
    dst_server_uuid: uuid().notNull(),
    dst_name: varchar({ length: 63 }).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'server_tunnel_connections_pk', columns: [cols.src_server_uuid, cols.dst_server_uuid] }),
    foreignKey({
      columns: [cols.dst_server_uuid, cols.dst_name],
      foreignColumns: [serverTunnelsTable.server_uuid, serverTunnelsTable.name],
    })
      .onUpdate('cascade')
      .onDelete('cascade'),
    uniqueIndex('server_tunnel_connections_src_server_uuid_dst_name_idx').on(cols.src_server_uuid, cols.dst_name),
    index('server_tunnel_connections_src_server_uuid_idx').on(cols.src_server_uuid),
    index('server_tunnel_connections_dst_server_uuid_idx').on(cols.dst_server_uuid),
  ],
);

export const serverFirewallsTable = pgTable('server_firewalls', {
  server_uuid: uuid()
    .references(() => serversTable.uuid, { onDelete: 'cascade' })
    .primaryKey()
    .notNull(),
  rules: jsonb().notNull(),
});

export const serverSubusersTable = pgTable(
  'server_subusers',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    permissions: varchar({ length: 64 }).array().notNull(),
    ignored_files: text().array().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'server_subusers_pk', columns: [cols.server_uuid, cols.user_uuid] }),
    index('server_subusers_server_uuid_idx').on(cols.server_uuid),
    index('server_subusers_user_uuid_idx').on(cols.user_uuid),
  ],
);

// plugged.host: the backup mails a person asked for on a server's Settings › Notifications
// (shared/src/backup_notifications.rs, docs/features/server-settings.md): one switch for backups made
// by hand, one for a schedule's. Owner or subuser; a person with both off has no row.
export const serverNotificationSettingsTable = pgTable(
  'server_notification_settings',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    manual_backups: boolean().default(false).notNull(),
    scheduled_backups: boolean().default(false).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'server_notification_settings_pk', columns: [cols.server_uuid, cols.user_uuid] }),
    index('server_notification_settings_user_uuid_idx').on(cols.user_uuid),
  ],
);

// plugged.host: the chat between a server's owner and its subusers (shared/src/server_chat.rs,
// docs/features/server-chat.md). One row per message; the server it belongs to, who wrote it and
// what they wrote.
export const serverChatMessagesTable = pgTable(
  'server_chat_messages',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    message: text().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_chat_messages_server_uuid_created_idx').on(cols.server_uuid, cols.created),
    index('server_chat_messages_user_uuid_idx').on(cols.user_uuid),
  ],
);

export const serverActivitiesTable = pgTable(
  'server_activities',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    impersonator_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    api_key_uuid: uuid().references(() => userApiKeysTable.uuid, { onDelete: 'set null' }),
    schedule_uuid: uuid().references(() => serverSchedulesTable.uuid, { onDelete: 'set null' }),
    event: varchar({ length: 255 }).notNull(),
    ip: inet(),
    data: jsonb().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_activities_server_uuid_created_idx').on(cols.server_uuid, cols.created),
    index('server_activities_impersonator_uuid_idx').on(cols.impersonator_uuid),
    index('server_activities_api_key_uuid_idx').on(cols.api_key_uuid),
    index('server_activities_schedule_uuid_idx').on(cols.schedule_uuid),
    index('server_activities_created_idx').on(cols.created),
    index('server_activities_user_uuid_created_idx').on(cols.user_uuid, cols.created),
  ],
);

export const serverVariablesTable = pgTable(
  'server_variables',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    variable_uuid: uuid()
      .references(() => nestEggVariablesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    value: text().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'server_variables_pk', columns: [cols.server_uuid, cols.variable_uuid] }),
    index('server_variables_server_uuid_idx').on(cols.server_uuid),
    index('server_variables_variable_uuid_idx').on(cols.variable_uuid),
  ],
);

export const serverMountsTable = pgTable(
  'server_mounts',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    mount_uuid: uuid()
      .references(() => mountsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'server_mounts_pk', columns: [cols.server_uuid, cols.mount_uuid] }),
    index('server_mounts_server_uuid_idx').on(cols.server_uuid),
    index('server_mounts_mount_uuid_idx').on(cols.mount_uuid),
  ],
);

export const serverDevicesTable = pgTable(
  'server_devices',
  {
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    device_uuid: uuid()
      .references(() => devicesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    primaryKey({ name: 'server_devices_pk', columns: [cols.server_uuid, cols.device_uuid] }),
    index('server_devices_server_uuid_idx').on(cols.server_uuid),
    index('server_devices_device_uuid_idx').on(cols.device_uuid),
  ],
);

export const serverBackupGroupsTable = pgTable(
  'server_backup_groups',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    order_: smallint().default(0).notNull(),
    retention: jsonb().default({ count: 0, days: 0, daily: 0, weekly: 0, monthly: 0, yearly: 0 }).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_backup_groups_server_uuid_idx').on(cols.server_uuid),
    uniqueIndex('server_backup_groups_server_uuid_name_idx').on(cols.server_uuid, cols.name),
  ],
);

export const serverBackupsTable = pgTable(
  'server_backups',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    server_uuid: uuid().references(() => serversTable.uuid, { onDelete: 'set null' }),
    node_uuid: uuid()
      .references(() => nodesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    backup_configuration_uuid: uuid().references(() => backupConfigurationsTable.uuid, { onDelete: 'set null' }),
    backup_group_uuid: uuid().references(() => serverBackupGroupsTable.uuid, { onDelete: 'set null' }),
    system_backup_policy_uuid: uuid().references(() => systemBackupPoliciesTable.uuid, { onDelete: 'set null' }),
    database_instance_uuid: uuid().references(() => serverDatabaseInstancesTable.uuid, { onDelete: 'set null' }),
    database_type: databaseAgentTypeEnum(),
    kind: serverBackupKindEnum().default('SERVER').notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    successful: boolean().default(false).notNull(),
    browsable: boolean().default(false).notNull(),
    streaming: boolean().default(false).notNull(),
    shared: boolean().default(false).notNull(),
    locked: boolean().default(false).notNull(),
    ignored_files: text().array().notNull(),
    checksum: varchar({ length: 255 }),
    bytes: bigint({ mode: 'number' }).default(0).notNull(),
    files: bigint({ mode: 'number' }).default(0).notNull(),
    disk: backupDiskEnum().notNull(),
    metadata: jsonb().default({}).notNull(),
    upload_id: text(),
    upload_path: text(),
    completed: timestamp(),
    deleting: timestamp(),
    deletion_retries: integer().default(0).notNull(),
    deleted: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_backups_server_uuid_idx').on(cols.server_uuid),
    index('server_backups_node_uuid_idx').on(cols.node_uuid),
    index('server_backups_node_uuid_created_idx').on(cols.node_uuid, cols.created).where(sql`${cols.deleted} IS NULL`),
    index('server_backups_backup_configuration_uuid_idx').on(cols.backup_configuration_uuid),
    index('server_backups_backup_group_uuid_idx').on(cols.backup_group_uuid),
    index('server_backups_system_backup_policy_uuid_idx').on(cols.system_backup_policy_uuid),
    index('server_backups_database_instance_uuid_idx').on(cols.database_instance_uuid),
    index('server_backups_successful_idx').on(cols.successful),
    index('server_backups_deleting_idx').on(cols.deleting),
    check('server_backups_kind_database_type_check', sql`(${cols.kind} = 'SERVER') = (${cols.database_type} IS NULL)`),
  ],
);

export const serverDatabasesTable = pgTable(
  'server_databases',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid)
      .notNull(),
    database_host_uuid: uuid()
      .references(() => databaseHostsTable.uuid)
      .notNull(),
    name: varchar({ length: 31 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    locked: boolean().default(false).notNull(),
    username: varchar({ length: 20 }).notNull(),
    password: bytea().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_databases_server_uuid_idx').on(cols.server_uuid),
    index('server_databases_database_host_uuid_idx').on(cols.database_host_uuid),
    uniqueIndex('server_databases_server_uuid_database_idx').on(cols.server_uuid, cols.name),
  ],
);

export const serverDatabaseInstancesTable = pgTable(
  'server_database_instances',
  {
    uuid: uuid().primaryKey().notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid)
      .notNull(),
    database_agent_host_uuid: uuid()
      .references(() => databaseAgentHostsTable.uuid)
      .notNull(),
    database_agent_template_uuid: uuid().references(() => databaseAgentTemplatesTable.uuid, {
      onDelete: 'set null',
    }),
    type: databaseAgentTypeEnum().notNull(),
    name: varchar({ length: 31 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    status: serverDatabaseInstanceStatusEnum(),
    locked: boolean().default(false).notNull(),
    template_version: integer(),
    image: text(),
    env: json(),
    memory: bigint({ mode: 'number' }),
    swap: bigint({ mode: 'number' }),
    disk: bigint({ mode: 'number' }),
    io_weight: smallint(),
    cpu: integer(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_database_instances_server_uuid_idx').on(cols.server_uuid),
    index('server_database_instances_database_agent_host_uuid_idx').on(cols.database_agent_host_uuid),
    uniqueIndex('server_database_instances_server_uuid_name_idx').on(cols.server_uuid, cols.name),
  ],
);

// plugged.host: a server's names under the domains, with the DNS records made for them and the
// tokens paid (refunded when it is deleted)
export const serverSubdomainsTable = pgTable(
  'server_subdomains',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    domain_uuid: uuid()
      .references(() => domainsTable.uuid)
      .notNull(),
    name: varchar({ length: 63 }).notNull(),
    record_type: varchar({ length: 8 }).notNull(),
    records: jsonb().default({}).notNull(),
    token_cost: integer().default(0).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_subdomains_server_uuid_idx').on(cols.server_uuid),
    index('server_subdomains_domain_uuid_idx').on(cols.domain_uuid),
    uniqueIndex('server_subdomains_domain_uuid_name_idx').on(cols.domain_uuid, cols.name),
  ],
);

// plugged.host: devboxes next to game servers (docs/features/devboxes.md). The game server and the
// person carry no foreign keys: a devbox owns a server Wings runs, which only its removal (through
// Wings) may delete, never a cascade.
export const devboxesTable = pgTable(
  'devboxes',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    game_server_uuid: uuid().notNull(),
    // NULL: the game server's shared devbox
    user_uuid: uuid(),
    server_uuid: uuid().references(() => serversTable.uuid, { onDelete: 'set null' }),
    node_uuid: uuid(),
    // the game server's volume on the node, given to Wings as the /server mount
    mount_source: varchar({ length: 4096 }),
    slug: varchar({ length: 24 }).notNull(),
    // sha256 of the bearer token of the current start, NULL while stopped
    token_hash: varchar({ length: 64 }),
    // the secret the start's connect codes come from, encrypted
    code_secret: text(),
    stopping: timestamp(),
    boot_status: varchar({ length: 32 }),
    boot_detail: varchar({ length: 255 }),
    host_key: varchar({ length: 100 }),
    version: varchar({ length: 40 }),
    connections: integer().default(0).notNull(),
    shared_rights: jsonb(),
    // who could sign in at the last start: a shared devbox starts over when one of them loses access
    people: uuid().array(),
    started: timestamp(),
    booted: timestamp(),
    last_seen: timestamp(),
    last_used: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('devboxes_slug_idx').on(cols.slug),
    uniqueIndex('devboxes_server_uuid_idx').on(cols.server_uuid),
    uniqueIndex('devboxes_token_hash_idx').on(cols.token_hash),
    uniqueIndex('devboxes_game_server_uuid_user_uuid_idx')
      .on(cols.game_server_uuid, cols.user_uuid)
      .where(sql`user_uuid IS NOT NULL`),
    uniqueIndex('devboxes_game_server_uuid_shared_idx').on(cols.game_server_uuid).where(sql`user_uuid IS NULL`),
    index('devboxes_user_uuid_idx').on(cols.user_uuid),
  ],
);

// plugged.host: people who may not use devboxes because of a critical abuse finding on one
export const devboxBlocksTable = pgTable(
  'devbox_blocks',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    // no foreign keys: the finding and its server may be deleted, the block stays
    finding_uuid: uuid().notNull(),
    server_uuid: uuid(),
    rule: varchar({ length: 64 }),
    lifted: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('devbox_blocks_user_uuid_finding_uuid_idx').on(cols.user_uuid, cols.finding_uuid),
    index('devbox_blocks_finding_uuid_idx').on(cols.finding_uuid),
  ],
);

// plugged.host: neo's chats (docs/features/neo.md). One per person and place (a server, or none
// for the dashboard) that is not archived; a server's chats go with the server.
export const neoConversationsTable = pgTable(
  'neo_conversations',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid()
      .references(() => usersTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    server_uuid: uuid().references(() => serversTable.uuid, { onDelete: 'cascade' }),
    title: varchar({ length: 120 * UTF8_MAX_SCALAR_SIZE }),
    // idle, running, awaiting_approval, limited, unlinked, error
    status: varchar({ length: 24 }).default('idle').notNull(),
    step_count: smallint().default(0).notNull(),
    tokens_used: bigint({ mode: 'number' }).default(0).notNull(),
    last_provider: varchar({ length: 64 }),
    last_error: varchar({ length: 255 }),
    last_activity: timestamp().defaultNow().notNull(),
    archived: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('neo_conversations_user_uuid_server_uuid_archived_idx').on(cols.user_uuid, cols.server_uuid, cols.archived),
    index('neo_conversations_last_activity_idx').on(cols.last_activity),
  ],
);

// plugged.host: a chat's messages in order: the user's, neo's (with its tool calls), one row per
// tool call (its status and output) and notices (limits, errors).
export const neoMessagesTable = pgTable(
  'neo_messages',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    conversation_uuid: uuid()
      .references(() => neoConversationsTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    seq: integer().notNull(),
    // user, assistant, tool, notice
    role: varchar({ length: 12 }).notNull(),
    content: text(),
    tool_calls: jsonb(),
    tool_call_id: varchar({ length: 64 }),
    tool_name: varchar({ length: 64 }),
    // tool rows: pending_approval, approved, queued, executing, done, failed, denied, skipped
    status: varchar({ length: 24 }),
    meta: jsonb(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('neo_messages_conversation_uuid_seq_idx').on(cols.conversation_uuid, cols.seq)],
);

// plugged.host: images attached to a neo chat message, shrunk to a JPEG on upload
// (docs/features/neo.md); they go with their message.
export const neoImagesTable = pgTable(
  'neo_images',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    message_uuid: uuid()
      .references(() => neoMessagesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    data: bytea().notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('neo_images_message_uuid_idx').on(cols.message_uuid)],
);

// plugged.host: each person's neo switches, the model picked in the chat and when they agreed to
// the notice. No row: on, asks first, the default model, not agreed yet.
export const userNeoSettingsTable = pgTable('user_neo_settings', {
  user_uuid: uuid()
    .references(() => usersTable.uuid, { onDelete: 'cascade' })
    .primaryKey()
    .notNull(),
  enabled: boolean().default(true).notNull(),
  auto_approve: boolean().default(false).notNull(),
  model: varchar({ length: 16 }),
  acknowledged: timestamp(),
  created: timestamp().defaultNow().notNull(),
});

// plugged.host: neo's 5-hour (short) and weekly (long) token windows when the panel meters neo
// itself, per user or per Discord account nobody linked.
export const neoUsageWindowsTable = pgTable(
  'neo_usage_windows',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'cascade' }),
    discord_id: varchar({ length: 20 }),
    kind: varchar({ length: 8 }).notNull(),
    started: timestamp().defaultNow().notNull(),
    resets: timestamp().defaultNow().notNull(),
    tokens_used: bigint({ mode: 'number' }).default(0).notNull(),
  },
  (cols) => [
    uniqueIndex('neo_usage_windows_user_uuid_kind_idx')
      .on(cols.user_uuid, cols.kind)
      .where(sql`user_uuid IS NOT NULL`),
    uniqueIndex('neo_usage_windows_discord_id_kind_idx')
      .on(cols.discord_id, cols.kind)
      .where(sql`user_uuid IS NULL`),
    index('neo_usage_windows_resets_idx').on(cols.resets),
  ],
);

// plugged.host: every metered neo turn (and failed ones, and provider tests), for the admin's view.
export const neoUsageEventsTable = pgTable(
  'neo_usage_events',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    discord_id: varchar({ length: 20 }),
    service: varchar({ length: 32 }).notNull(),
    conversation: varchar({ length: 64 }),
    provider_slot: smallint(),
    provider_label: varchar({ length: 64 }),
    driver: varchar({ length: 16 }),
    model: varchar({ length: 128 }),
    prompt_tokens: integer().default(0).notNull(),
    completion_tokens: integer().default(0).notNull(),
    charged_tokens: bigint({ mode: 'number' }).default(0).notNull(),
    finish_reason: varchar({ length: 24 }),
    // ok, failed, test
    status: varchar({ length: 16 }).notNull(),
    attempts: jsonb(),
    duration_ms: integer().default(0).notNull(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('neo_usage_events_user_uuid_created_idx').on(cols.user_uuid, cols.created),
    index('neo_usage_events_discord_id_idx').on(cols.discord_id),
    index('neo_usage_events_created_idx').on(cols.created),
  ],
);

// plugged.host: neo on Discord's short memory (shared/src/neo/discord, docs/features/neo-discord.md).
// One conversation is one person in one channel or DM (`{channel_id}:{author_id}`). A `user` row is a
// message someone sent the assistant bot (`pending` until it is answered); the rows of one answer
// (the messages it answers, the bot's turns and its lookups) share `turn_uuid`, the uuid of the
// first message it answers, in `position` order. Deleted after neo's retention days.
export const neoDiscordMessagesTable = pgTable(
  'neo_discord_messages',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    conversation: varchar({ length: 48 }).notNull(),
    turn_uuid: uuid(),
    position: smallint().default(0).notNull(),
    // the Discord message a user row is, to reply to
    discord_message_id: varchar({ length: 20 }),
    channel_id: varchar({ length: 20 }).notNull(),
    // null in a DM
    guild_id: varchar({ length: 20 }),
    author_id: varchar({ length: 20 }).notNull(),
    author_name: varchar({ length: 100 * UTF8_MAX_SCALAR_SIZE }),
    // user, assistant, tool
    role: varchar({ length: 12 }).notNull(),
    content: text(),
    tool_calls: jsonb(),
    tool_call_id: varchar({ length: 64 }),
    tool_name: varchar({ length: 64 }),
    // pending, running, done, failed
    status: varchar({ length: 12 }).notNull(),
    // user rows: mentions, images, reply_to, and who answered (linked, account_id, user_uuid, error)
    meta: jsonb(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('neo_discord_messages_conversation_created_idx').on(cols.conversation, cols.created),
    index('neo_discord_messages_turn_uuid_idx').on(cols.turn_uuid),
    index('neo_discord_messages_created_idx').on(cols.created),
  ],
);

// plugged.host: rux, the moderation bot (shared/src/rux, docs/features/rux.md). Its rows have
// numbers instead of uuids: people and the model name cases, giveaways and rules by them ("case
// 12"). Discord ids are text, as Discord sends them. The account columns say who acted: the
// noprofit.space account id while noprofit.space is connected, else the panel user.

// rux's short memory: every order (a `user` row, `pending` until its worker claims it), the model's
// answers and each tool call's output; the rows of one order share its turn_id.
export const ruxMessagesTable = pgTable(
  'rux_messages',
  {
    id: bigint({ mode: 'number' }).primaryKey().generatedByDefaultAsIdentity(),
    // `<channel id>:<author id>`
    conversation: varchar({ length: 80 }).notNull(),
    turn_id: bigint({ mode: 'number' }),
    discord_message_id: varchar({ length: 32 }),
    channel_id: varchar({ length: 32 }).notNull(),
    // none: a DM
    guild_id: varchar({ length: 32 }),
    author_id: varchar({ length: 32 }).notNull(),
    author_name: varchar({ length: 100 * UTF8_MAX_SCALAR_SIZE }),
    // user, assistant, tool
    role: varchar({ length: 16 }).notNull(),
    content: text(),
    tool_calls: jsonb(),
    tool_call_id: varchar({ length: 64 }),
    tool_name: varchar({ length: 64 }),
    // pending, running, done, failed, ignored
    status: varchar({ length: 16 }).default('done').notNull(),
    meta: jsonb(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('rux_messages_conversation_idx').on(cols.conversation),
    index('rux_messages_turn_id_idx').on(cols.turn_id),
    index('rux_messages_role_status_idx').on(cols.role, cols.status),
    index('rux_messages_created_idx').on(cols.created),
  ],
);

// plugged.host: the moderation log: warnings, timeouts, kicks, bans (with their end), unbans and
// cleared warnings, by an order or by rux itself (`automatic`, with the rule that caught it).
export const ruxCasesTable = pgTable(
  'rux_cases',
  {
    id: bigint({ mode: 'number' }).primaryKey().generatedByDefaultAsIdentity(),
    guild_id: varchar({ length: 32 }).notNull(),
    action: varchar({ length: 24 }).notNull(),
    target_id: varchar({ length: 32 }),
    target_name: varchar({ length: 100 * UTF8_MAX_SCALAR_SIZE }),
    channel_id: varchar({ length: 32 }),
    moderator_discord_id: varchar({ length: 32 }).notNull(),
    moderator_account_id: bigint({ mode: 'number' }),
    moderator_user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    reason: varchar({ length: 500 * UTF8_MAX_SCALAR_SIZE }),
    duration_seconds: integer(),
    automatic: boolean().default(false).notNull(),
    rule_id: bigint({ mode: 'number' }).references(() => ruxRulesTable.id, { onDelete: 'set null' }),
    // a temporary ban's end, lifted by rux
    expires: timestamp(),
    resolved: timestamp(),
    revoked: timestamp(),
    meta: jsonb(),
    created: timestamp().defaultNow().notNull(),
    updated: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('rux_cases_guild_id_target_id_idx').on(cols.guild_id, cols.target_id),
    index('rux_cases_action_expires_resolved_idx').on(cols.action, cols.expires, cols.resolved),
    index('rux_cases_created_idx').on(cols.created),
  ],
);

// plugged.host: giveaways of credits, drawn and paid when they end.
export const ruxGiveawaysTable = pgTable(
  'rux_giveaways',
  {
    id: bigint({ mode: 'number' }).primaryKey().generatedByDefaultAsIdentity(),
    guild_id: varchar({ length: 32 }).notNull(),
    channel_id: varchar({ length: 32 }).notNull(),
    message_id: varchar({ length: 32 }),
    prize: varchar({ length: 120 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    // per winner
    credits: bigint({ mode: 'number' }).notNull(),
    winners_count: smallint().default(1).notNull(),
    ends: timestamp().notNull(),
    // the winners of the last this many giveaways may not enter
    exclude_recent: smallint().default(0).notNull(),
    // Discord ids kept out
    excluded: jsonb(),
    // active, drawing, ended, cancelled
    status: varchar({ length: 16 }).default('active').notNull(),
    created_by_discord_id: varchar({ length: 32 }).notNull(),
    created_by_account_id: bigint({ mode: 'number' }),
    created_by_user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    ended: timestamp(),
    // none, pending, paid, failed
    payout_status: varchar({ length: 16 }).default('none').notNull(),
    payout_attempts: integer().default(0).notNull(),
    payout_error: varchar({ length: 255 }),
    // the giveaway's id where the credits were paid (noprofit.space's), when it has one
    payout_reference: varchar({ length: 64 }),
    rerolls: smallint().default(0).notNull(),
    // the entry count the message shows
    rendered_entries: integer().default(0).notNull(),
    created: timestamp().defaultNow().notNull(),
    updated: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('rux_giveaways_ends_idx').on(cols.ends),
    index('rux_giveaways_status_idx').on(cols.status),
    index('rux_giveaways_payout_status_idx').on(cols.payout_status),
  ],
);

export const ruxGiveawayEntriesTable = pgTable(
  'rux_giveaway_entries',
  {
    id: bigint({ mode: 'number' }).primaryKey().generatedByDefaultAsIdentity(),
    giveaway_id: bigint({ mode: 'number' })
      .references(() => ruxGiveawaysTable.id, { onDelete: 'cascade' })
      .notNull(),
    discord_id: varchar({ length: 32 }).notNull(),
    username: varchar({ length: 64 * UTF8_MAX_SCALAR_SIZE }),
    won: timestamp(),
    // which draw it won in (0 the first, then rerolls)
    draw: smallint(),
    // pending, paid, failed
    payout: varchar({ length: 16 }),
    created: timestamp().defaultNow().notNull(),
    updated: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    uniqueIndex('rux_giveaway_entries_giveaway_id_discord_id_idx').on(cols.giveaway_id, cols.discord_id),
    index('rux_giveaway_entries_discord_id_idx').on(cols.discord_id),
  ],
);

// plugged.host: the roles whose members may give rux moderation orders on their server, and the
// Discord permissions rux added to them (taken away again with the role).
export const ruxModeratorRolesTable = pgTable(
  'rux_moderator_roles',
  {
    id: bigint({ mode: 'number' }).primaryKey().generatedByDefaultAsIdentity(),
    guild_id: varchar({ length: 32 }).notNull(),
    role_id: varchar({ length: 32 }).notNull(),
    role_name: varchar({ length: 100 * UTF8_MAX_SCALAR_SIZE }),
    granted_by_discord_id: varchar({ length: 32 }).notNull(),
    granted_by_account_id: bigint({ mode: 'number' }),
    granted_by_user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    // Discord's permission bits, as a decimal string
    permissions_added: varchar({ length: 32 }).default('0').notNull(),
    created: timestamp().defaultNow().notNull(),
    updated: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('rux_moderator_roles_guild_id_role_id_idx').on(cols.guild_id, cols.role_id)],
);

// plugged.host: standing rules: what a member's message must not do (`description`, and what looks
// close but is fine, `exceptions`), the keywords that get a message checked outside watched
// channels, and what rux does then.
export const ruxRulesTable = pgTable(
  'rux_rules',
  {
    id: bigint({ mode: 'number' }).primaryKey().generatedByDefaultAsIdentity(),
    guild_id: varchar({ length: 32 }).notNull(),
    description: varchar({ length: 600 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    exceptions: varchar({ length: 600 * UTF8_MAX_SCALAR_SIZE }),
    keywords: jsonb().default([]).notNull(),
    // warn, timeout, delete, kick, ban
    action: varchar({ length: 16 }).notNull(),
    duration_seconds: integer(),
    delete_message: boolean().default(false).notNull(),
    reason: varchar({ length: 300 * UTF8_MAX_SCALAR_SIZE }),
    // none: every channel
    channel_ids: jsonb(),
    active: boolean().default(true).notNull(),
    created_by_discord_id: varchar({ length: 32 }).notNull(),
    created_by_account_id: bigint({ mode: 'number' }),
    created_by_user_uuid: uuid().references(() => usersTable.uuid, { onDelete: 'set null' }),
    hits: integer().default(0).notNull(),
    actions: integer().default(0).notNull(),
    last_action: timestamp(),
    created: timestamp().defaultNow().notNull(),
    updated: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('rux_rules_guild_id_active_idx').on(cols.guild_id, cols.active)],
);

// plugged.host: channels where every message is checked (by name or id), per server, and the
// messages waiting for their check.
export const ruxWatchesTable = pgTable(
  'rux_watches',
  {
    id: bigint({ mode: 'number' }).primaryKey().generatedByDefaultAsIdentity(),
    guild_id: varchar({ length: 32 }).notNull(),
    channel: varchar({ length: 100 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    added_by_discord_id: varchar({ length: 32 }),
    created: timestamp().defaultNow().notNull(),
    updated: timestamp().defaultNow().notNull(),
  },
  (cols) => [uniqueIndex('rux_watches_guild_id_channel_idx').on(cols.guild_id, cols.channel)],
);

export const ruxWatchItemsTable = pgTable(
  'rux_watch_items',
  {
    id: bigint({ mode: 'number' }).primaryKey().generatedByDefaultAsIdentity(),
    guild_id: varchar({ length: 32 }),
    message_id: varchar({ length: 32 }).notNull(),
    channel_id: varchar({ length: 32 }).notNull(),
    author_id: varchar({ length: 32 }).notNull(),
    author_name: varchar({ length: 100 * UTF8_MAX_SCALAR_SIZE }),
    roles: jsonb(),
    content: text().notNull(),
    // the message it replies to, for the check's context
    replied_to: text(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('rux_watch_items_created_idx').on(cols.created)],
);

export const serverSchedulesTable = pgTable(
  'server_schedules',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    server_uuid: uuid()
      .references(() => serversTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    name: varchar({ length: 255 * UTF8_MAX_SCALAR_SIZE }).notNull(),
    enabled: boolean().notNull(),
    triggers: jsonb().notNull(),
    condition: jsonb().notNull(),
    last_run: timestamp(),
    last_failure: timestamp(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [
    index('server_schedules_server_uuid_idx').on(cols.server_uuid),
    index('server_schedules_enabled_idx').on(cols.enabled),
    uniqueIndex('server_schedules_server_uuid_name_idx').on(cols.server_uuid, cols.name),
  ],
);

export const serverScheduleStepsTable = pgTable(
  'server_schedule_steps',
  {
    uuid: uuid().default(sql`gen_random_uuid()`).primaryKey().notNull(),
    schedule_uuid: uuid()
      .references(() => serverSchedulesTable.uuid, { onDelete: 'cascade' })
      .notNull(),
    action: jsonb().notNull(),
    order_: smallint().default(0).notNull(),
    error: text(),
    created: timestamp().defaultNow().notNull(),
  },
  (cols) => [index('server_schedule_steps_schedule_uuid_idx').on(cols.schedule_uuid)],
);
