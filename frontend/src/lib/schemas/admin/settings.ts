import { z } from 'zod';
import { NEO_MAX_PROVIDERS, parseJsonHeaders, parseJsonObject } from '@/lib/neo.ts'; // plugged.host
import { oobeStepKey } from '@/lib/schemas/oobe.ts';
import { ruleMessage } from '@/lib/schemas/rules.ts';
import { serverTrashSettingsSchema } from '@/lib/schemas/server/settings.ts';
import { twoFactorMethod } from '@/lib/schemas/user.ts';
import { nullableNumber, nullableString } from '@/lib/serialization/transformers.ts';
import { getTranslations } from '@/providers/contexts/translationContext.ts'; // plugged.host
import { eggConfigurationRouteItemSchema, hostnameSchema } from '../generic.ts';

export const adminSettingsApplicationSchema = z.object({
  name: z.string().min(1).max(64),
  icon: z.string().min(1).max(255),
  iconLight: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  banner: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  bannerLight: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  url: z.url({ protocol: /^https?$/ }).max(255),
  additionalUrls: z
    .array(z.string())
    .max(32)
    .refine(
      (urls) =>
        urls.every(
          (url) =>
            z
              .url({ protocol: /^https?$/ })
              .max(255)
              .safeParse(url).success,
        ),
      {
        message: 'Every URL must be a valid http or https URL',
      },
    ),
  language: z.string(),
  twoFactorRequirement: z.enum(['admins', 'all_users', 'none']),
  emailTwoFactorEnabled: z.boolean(),
  twoFactorAcceptedMethods: z.array(twoFactorMethod),
  emailVerificationRequired: z.boolean(),
  sessionCookie: z.string().min(1).max(255),
  sessionDurationSeconds: z.number().min(60).max(31536000),
  telemetryEnabled: z.boolean(),
  registrationEnabled: z.boolean(),
  passwordLoginEnabled: z.boolean(),
});

export const adminSettingsCaptchaProviderNoneSchema = z.object({
  type: z.literal('none'),
});

export const adminSettingsCaptchaProviderTurnstileSchema = z.object({
  type: z.literal('turnstile'),
  siteKey: z.string().min(1).max(255),
  secretKey: z.string().min(1).max(255),
});

export const adminSettingsCaptchaProviderRecaptchaSchema = z.object({
  type: z.literal('recaptcha'),
  siteKey: z.string().min(1).max(255),
  secretKey: z.string().min(1).max(255),
  v3: z.boolean(),
  threshold: z.number().min(0).max(1).default(0.5),
});

export const adminSettingsCaptchaProviderHcaptchaSchema = z.object({
  type: z.literal('hcaptcha'),
  siteKey: z.string().min(1).max(255),
  secretKey: z.string().min(1).max(255),
});

export const adminSettingsCaptchaProviderFriendlyCaptchaSchema = z.object({
  type: z.literal('friendly_captcha'),
  siteKey: z.string().min(1).max(255),
  apiKey: z.string().min(1).max(255),
});

export const adminSettingsCaptchaProviderCapSchema = z.object({
  type: z.literal('cap'),
  apiUrl: z
    .url({ protocol: /^https?$/ })
    .max(255)
    .refine(
      (value) => {
        if (!URL.canParse(value)) return false;
        const url = new URL(value);
        return !url.username && !url.password && !value.includes('?') && !value.includes('#');
      },
      { message: 'The URL must not contain credentials, a query, or a fragment' },
    ),
  siteKey: z
    .string()
    .min(1)
    .max(255)
    .regex(
      /^[a-zA-Z0-9_-]+$/,
      ruleMessage((t) => t('common.form.rule.captchaSiteKey', {})),
    ),
  secretKey: z.string().min(1).max(255),
});

export const adminSettingsCaptchaProviderSchema = z.discriminatedUnion('type', [
  adminSettingsCaptchaProviderNoneSchema,
  adminSettingsCaptchaProviderTurnstileSchema,
  adminSettingsCaptchaProviderRecaptchaSchema,
  adminSettingsCaptchaProviderHcaptchaSchema,
  adminSettingsCaptchaProviderFriendlyCaptchaSchema,
  adminSettingsCaptchaProviderCapSchema,
]);

export const adminSettingsEmailNoneSchema = z.object({
  type: z.literal('none'),
});

export const adminSettingsEmailSmtpSchema = z.object({
  type: z.literal('smtp'),
  host: hostnameSchema,
  port: z.number().min(1),
  username: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  password: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  tlsMode: z.enum(['none', 'start_tls', 'implicit_tls']),
  skipCertValidation: z.boolean(),
  heloDomain: z.preprocess(nullableString, hostnameSchema.nullable()),
  fromAddress: z.email().max(255),
  fromName: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
});

export const adminSettingsEmailSendmailSchema = z.object({
  type: z.literal('sendmail'),
  command: z.string().min(1).max(255),
  fromAddress: z.email().max(255),
  fromName: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
});

export const adminSettingsEmailFilesystemSchema = z.object({
  type: z.literal('filesystem'),
  path: z.string().min(1).max(255),
  fromAddress: z.email().max(255),
  fromName: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
});

export const adminSettingsEmailSchema = z.discriminatedUnion('type', [
  adminSettingsEmailNoneSchema,
  adminSettingsEmailSmtpSchema,
  adminSettingsEmailSendmailSchema,
  adminSettingsEmailFilesystemSchema,
]);

export const adminSettingsEmailTestSchema = z.object({
  email: z.email().max(255),
});

export const adminSettingsEmailTemplateListSchema = z.array(
  z.object({
    identifier: z.string(),
    availableVariables: z.array(z.string()),
  }),
);

export const adminSettingsEmailTemplateSchema = z.object({
  availableVariables: z.array(z.string()),
  defaultEnabled: z.boolean(),
  enabled: z.boolean(),
  defaultSubject: z.string(),
  subject: z.string().nullable(),
  defaultContent: z.string(),
  content: z.string().nullable(),
});

export const adminSettingsEmailTemplateUpdateSchema = z.object({
  content: z.string().min(1).nullable().optional(),
  subject: z.string().min(1).max(255).nullable().optional(),
  enabled: z.boolean().optional(),
});

export const adminSettingsEmailVariableSchema = z.object({
  templateIdentifier: z.string().nullable(),
  name: z.string(),
  system: z.boolean(),
  defaultValue: z.string().nullable(),
  defaultTranslations: z.record(z.string(), z.string()),
  value: z.string().nullable(),
  valueTranslations: z.record(z.string(), z.string()),
});

export const adminSettingsEmailVariableCreateSchema = z.object({
  name: z
    .string()
    .min(1)
    .max(64)
    .regex(
      /^[a-z][a-z0-9_]{0,63}$/,
      ruleMessage((t) => t('common.form.rule.variableName', {})),
    ),
  value: z.string().min(1).max(8192),
  valueTranslations: z.record(z.string(), z.string().min(1).max(8192)),
});

export const adminSettingsEmailVariableUpdateSchema = z.object({
  value: z.string().min(1).max(8192).nullable().optional(),
  valueTranslations: z.record(z.string(), z.string().min(1).max(8192)).optional(),
});

export const adminSettingsServerSchema = z.object({
  maxFileManagerViewSize: z.number().min(0),
  maxFileManagerContentSearchSize: z.number().min(0),
  maxFileManagerSearchResults: z.number().min(1),
  maxSubuserCount: z.number().min(0),
  maxScheduleStepCount: z.number().min(0),
  maxBackupGroupCount: z.number().min(0),
  maxFirewallRuleCount: z.number().min(0),
  maxFirewallRuleSourceCount: z.number().min(0),
  maxTunnelConnectionCount: z.number().min(0),
  maxTunnelPortCount: z.number().min(0),
  maxDatabaseInstanceDatabaseCount: z.number().min(0),
  maxDatabaseInstanceUserCount: z.number().min(0),
  allowOverwritingCustomDockerImage: z.boolean(),
  allowViewingInstallationLogs: z.boolean(),
  allowAcknowledgingInstallationFailure: z.boolean(),
  allowViewingTransferProgress: z.boolean(),
  trash: serverTrashSettingsSchema,
  allowChangingTrashSettings: z.boolean(),
  containerPrelude: z.string().min(1).max(255),
});

export const adminSettingsUserSchema = z.object({
  maxServerGroupCount: z.number().min(0),
  maxApiKeyCount: z.number().min(0),
  maxCommandSnippetCount: z.number().min(0),
  maxSecurityKeyCount: z.number().min(0),
  maxSshKeyCount: z.number().min(0),
  maxSettingsCount: z.number().min(0),
  maxSettingsValueBytes: z.number().min(0),
  allowChangingLanguage: z.boolean(),
  allowChangingAvatar: z.boolean(),
  routeOrder: z.array(eggConfigurationRouteItemSchema).max(100).nullable(),
});

export const adminSettingsActivitySchema = z.object({
  adminLogRetentionDays: z.number().min(1).max(3650),
  adminLogRetentionCount: z.preprocess(nullableNumber, z.number().min(1).nullable()),
  userLogRetentionDays: z.number().min(1).max(3650),
  userLogRetentionCount: z.preprocess(nullableNumber, z.number().min(1).nullable()),
  serverLogRetentionDays: z.number().min(1).max(3650),
  serverLogRetentionCount: z.preprocess(nullableNumber, z.number().min(1).nullable()),
  serverLogAdminActivity: z.boolean(),
  serverLogScheduleActivity: z.boolean(),
  serverHideActivityIps: z.enum(['admins', 'all_users', 'none']),
});

export const adminSettingsRatelimitConfigurationSchema = z.object({
  hits: z.number().min(1),
  windowSeconds: z.number().min(1),
});

export const adminSettingsRatelimitsSchema = z.object({
  authRegister: adminSettingsRatelimitConfigurationSchema,
  authLogin: adminSettingsRatelimitConfigurationSchema,
  authLoginCheckpoint: adminSettingsRatelimitConfigurationSchema,
  authLoginCheckpointEmail: adminSettingsRatelimitConfigurationSchema,
  authLoginSecurityKey: adminSettingsRatelimitConfigurationSchema,
  authPasswordForgot: adminSettingsRatelimitConfigurationSchema,
  authPasswordReset: adminSettingsRatelimitConfigurationSchema,
  authEmailVerification: adminSettingsRatelimitConfigurationSchema,
  client: adminSettingsRatelimitConfigurationSchema,
  clientAccountEmailResendVerification: adminSettingsRatelimitConfigurationSchema,
  clientServersBackupsCreate: adminSettingsRatelimitConfigurationSchema,
  clientServersFilesPull: adminSettingsRatelimitConfigurationSchema,
  clientServersFilesPullQuery: adminSettingsRatelimitConfigurationSchema,
  remote: adminSettingsRatelimitConfigurationSchema,
  remoteSftpAuth: adminSettingsRatelimitConfigurationSchema,
  remoteEnroll: adminSettingsRatelimitConfigurationSchema,
  exemptIps: z.array(z.ipv4().or(z.ipv6()).or(z.cidrv4()).or(z.cidrv6())).max(256),
  exemptApiKeys: z.array(z.uuid()).max(256),
});

export const adminSettingsStorageFilesystemSchema = z.object({
  type: z.literal('filesystem'),
  path: z.string().min(1).max(255),
});

export const adminSettingsStorageS3Schema = z.object({
  type: z.literal('s3'),
  accessKey: z.string().min(1).max(512),
  secretKey: z.string().min(1).max(512),
  bucket: z.string().min(1).max(63),
  region: z.string().min(1).max(63),
  publicUrl: z.url({ protocol: /^https?$/ }).max(255),
  endpoint: z.string().min(1).max(255),
  pathStyle: z.boolean(),
});

export const adminSettingsStorageSchema = z.discriminatedUnion('type', [
  adminSettingsStorageFilesystemSchema,
  adminSettingsStorageS3Schema,
]);

export const adminSettingsWebauthnSchema = z.object({
  enabled: z.boolean(),
  allowDiscoverable: z.boolean(),
  rpId: z.string().min(1).max(255),
  rpOrigin: z.url({ protocol: /^https?$/ }).max(255),
  authenticationTimeoutSeconds: z.number().min(30).max(600),
  registrationTimeoutSeconds: z.number().min(30).max(600),
});

export const adminSettingsMetadataSchema = z.object({
  description: z.preprocess(nullableString, z.string().min(1).max(320).nullable()),
  themeColor: z.string().regex(
    /^#[0-9a-fA-F]{6}$/,
    ruleMessage((t) => t('common.form.rule.hexColor', {})),
  ),
  ogImage: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  twitterCard: z.enum(['summary', 'summary_large_image']),
  indexable: z.boolean(),
  seasonalTheme: z.enum(['default', 'halloween', 'christmas']), // plugged.host
});

// plugged.host: noprofit.space (shared/src/settings/noprofit.rs). A stored secret comes back as an
// empty string and is kept when sent back empty.
export const adminSettingsNoprofitSchema = z.object({
  enabled: z.boolean(),
  accountsUrl: z.url({ protocol: /^https?$/ }).max(255),
  ssoClient: z.string().max(64),
  ssoAudience: z.preprocess(nullableString, z.string().min(1).max(128).nullable()),
  ssoCallbackUrl: z.preprocess(
    nullableString,
    z
      .url({ protocol: /^https?$/ })
      .max(255)
      .nullable(),
  ),
  adminsOnly: z.boolean(),
  linkByEmail: z.boolean(),
  deletedAccountDays: z.number().int().min(0).max(365),
  ssoSecret: z.string().max(512).nullable(),
  apiSecret: z.string().max(512).nullable(),
  neoSecret: z.string().max(512).nullable(),
});

// plugged.host: the noprofit.space tab's form; removing the neo secret puts neo back on this panel's models
export const adminSettingsNoprofitFormSchema = adminSettingsNoprofitSchema.extend({
  clearNeoSecret: z.boolean(),
});

// plugged.host: the status page (shared/src/settings/status.rs, docs/features/status.md)
export const adminSettingsStatusSchema = z.object({
  enabled: z.boolean(),
  siteUrl: z.preprocess(
    nullableString,
    z
      .url({ protocol: /^https?$/ })
      .max(255)
      .nullable(),
  ),
  siteHost: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  panelUrl: z.preprocess(
    nullableString,
    z
      .url({ protocol: /^https?$/ })
      .max(255)
      .nullable(),
  ),
  pageUrl: z.preprocess(
    nullableString,
    z
      .url({ protocol: /^https?$/ })
      .max(255)
      .nullable(),
  ),
  historyDays: z.number().int().min(1).max(365),
  cpuThreshold: z.number().int().min(1).max(100),
});

// plugged.host: abuse protection (shared/src/settings/abuse.rs, docs/features/abuse.md). A stored
// webhook comes back as an empty string and is kept when sent back empty.
export const adminSettingsAbuseSchema = z.object({
  mode: z.enum(['suspend', 'report', 'off']),
  alertWebhook: z.union([z.literal(''), z.url({ protocol: /^https$/ }).max(512)]).nullable(),
  cpuPercent: z.number().int().min(50).max(100),
  cpuMinutes: z.number().int().min(5).max(1440),
  deleteAfterHours: z.number().int().min(0).max(8760),
  // the panel's own account standing (shared/src/standing.rs, docs/features/standing.md)
  standingLimitedAt: z.number().int().min(1).max(100),
  standingLimitedDays: z.number().int().min(1).max(3650),
  standingVeryLimitedAt: z.number().int().min(1).max(100),
  standingVeryLimitedDays: z.number().int().min(1).max(3650),
  standingSuspendedAt: z.number().int().min(1).max(100),
});

// plugged.host: what players may create (shared/src/settings/games.rs, docs/features/create-server.md)
export const adminSettingsGameVariantSchema = z.object({
  value: z.string().min(1).max(64),
  label: z.string().min(1).max(64),
  description: z.preprocess(nullableString, z.string().max(255).nullable()),
  startup: z.preprocess(nullableString, z.string().max(8192).nullable()),
  image: z.preprocess(nullableString, z.string().max(255).nullable()),
});

export const adminSettingsGameSchema = z.object({
  eggUuid: z.uuid(),
  name: z.preprocess(nullableString, z.string().max(64).nullable()),
  description: z.preprocess(nullableString, z.string().max(255).nullable()),
  icon: z.preprocess(nullableString, z.string().max(1024).nullable()),
  variable: z.preprocess(nullableString, z.string().max(255).nullable()),
  variants: z.array(adminSettingsGameVariantSchema).max(32),
});

export const adminSettingsGamesSchema = z.object({
  createEnabled: z.boolean(),
  catalog: z.array(adminSettingsGameSchema).max(100),
  limitAllocations: z.number().int().min(1).max(100),
  limitDatabases: z.number().int().min(0).max(100),
  limitBackups: z.number().int().min(0).max(100),
  limitSchedules: z.number().int().min(0).max(100),
  allowResizing: z.boolean(),
  allowDeleting: z.boolean(),
  transferEnabled: z.boolean(),
  transferCooldownHours: z.number().int().min(0).max(720),
  transferAttempts: z.number().int().min(1).max(5),
  transferParallel: z.number().int().min(1).max(10),
  transferStallMinutes: z.number().int().min(2).max(120),
  transferFinalizeMinutes: z.number().int().min(5).max(240),
  maxServers: z.number().int().min(0).max(1000),
  allowanceCpu: z.number().int().min(0),
  allowanceMemory: z.number().int().min(0),
  allowanceDisk: z.number().int().min(0),
});

// plugged.host: Admin › Settings › Hosting › Minecraft, part of the games section; an empty key
// keeps the stored one (docs/features/minecraft.md)
export const adminSettingsMinecraftSchema = z.object({
  minecraftCurseforgeKey: z.string().max(255),
  minecraftSpigot: z.boolean(),
  minecraftUserAgent: z.string().max(255),
  minecraftLogCheck: z.boolean(),
});

// plugged.host: Admin › Settings › Hosting › FiveM, part of the games section (docs/features/fivem.md)
export const adminSettingsFivemSchema = z.object({
  fivemImage: z.string().max(255).regex(/^\S*$/),
  fivemEnhanced: z.boolean(),
});

// plugged.host: the games section as the panel sends it: the Games tab's fields and the Minecraft
// and FiveM tabs' (a stored key comes back as a placeholder)
export const adminSettingsGamesStoreSchema = adminSettingsGamesSchema.extend({
  minecraftCurseforgeKey: z.string().max(255).nullable(),
  minecraftSpigot: z.boolean(),
  minecraftUserAgent: z.string().max(255),
  minecraftLogCheck: z.boolean(),
  fivemImage: z.string().max(255),
  fivemEnhanced: z.boolean(),
});

// plugged.host: server subdomains on Cloudflare (shared/src/settings/subdomains.rs, docs/features/subdomains.md);
// an empty token keeps the stored one
export const adminSettingsSubdomainsSchema = z.object({
  enabled: z.boolean(),
  cloudflareToken: z.string().max(255),
  defaultLimit: z.number().int().min(0).max(100),
  costFirst: z.number().int().min(0).max(1_000_000),
  costSecond: z.number().int().min(0).max(1_000_000),
  costThird: z.number().int().min(0).max(1_000_000),
  costMore: z.number().int().min(0).max(1_000_000),
  blockedNames: z.string().max(4096),
});

// plugged.host: the subdomains section as the panel sends it (a stored token comes back as a placeholder)
export const adminSettingsSubdomainsStoreSchema = adminSettingsSubdomainsSchema.extend({
  cloudflareToken: z.string().max(255).nullable(),
});

// plugged.host: Discord membership, invite and boost rewards, the bots (shared/src/settings/discord.rs,
// docs/features/discord.md); an empty token keeps the stored one
export const adminSettingsDiscordSchema = z.object({
  enabled: z.boolean(),
  guildId: z.string().regex(/^(\d{15,20})?$/),
  botToken: z.string().max(255),
  inviteUrl: z.union([z.literal(''), z.url({ protocol: /^https$/ }).max(255)]),
  oauthProviderUuid: z.string().nullable(),
  tierCpu: z.number().int().min(0).max(3200),
  tierMemory: z.number().int().min(0).max(262144),
  tierDisk: z.number().int().min(0).max(1048576),
  creditsPerInvite: z.number().int().min(0).max(1_000_000),
  minAccountAgeDays: z.number().int().min(0).max(3650),
  inviteMaturityHours: z.number().int().min(0).max(720),
  inviteClawback: z.boolean(),
  boostOneCredits: z.number().int().min(0).max(10_000_000),
  boostTwoCredits: z.number().int().min(0).max(10_000_000),
  boostTwoRoleId: z.string().regex(/^(\d{15,20})?$/),
  subdomainsRequireLink: z.boolean(),
  subdomainsRequireMember: z.boolean(),
  rewardsBotEnabled: z.boolean(),
  assistantBotEnabled: z.boolean(),
  assistantBotToken: z.string().max(255),
  moderationBotEnabled: z.boolean(),
  moderationBotToken: z.string().max(255),
});

// plugged.host: the Discord section as the panel sends it (stored tokens come back as placeholders)
export const adminSettingsDiscordStoreSchema = adminSettingsDiscordSchema.extend({
  botToken: z.string().max(255).nullable(),
  assistantBotToken: z.string().max(255).nullable(),
  moderationBotToken: z.string().max(255).nullable(),
});

// plugged.host: rux, the moderation bot (shared/src/settings/rux.rs, docs/features/rux.md)
const discordId = z.string().regex(/^(\d{15,20})?$/);
const ruxDomains = z.array(z.string().max(253)).max(20);
const ruxDuration = z
  .string()
  .max(16)
  .regex(/^\s*(\d+\s*[smhdw])+\s*$/i);

export const adminSettingsRuxServerSchema = z.object({
  id: z.string().regex(/^\d{15,20}$/),
  name: z.string().max(64),
  about: z.string().max(255),
  giveawayChannelId: discordId,
  modLogChannelId: discordId,
  trustedDomains: ruxDomains,
});

// an empty secret keeps the stored one
export const adminSettingsRuxSchema = z.object({
  name: z
    .string()
    .min(1)
    .max(32)
    .regex(/^[\p{L}\p{N}_-]+$/u),
  guildId: discordId,
  giveawayChannelId: discordId,
  modLogChannelId: discordId,
  trustedDomains: ruxDomains,
  servers: z.array(adminSettingsRuxServerSchema).max(9),
  timezone: z.string().min(1).max(64),
  locale: z.enum(['en', 'de']),
  giveawayMaxCredits: z.number().int().min(1).max(1_000_000),
  giveawayMaxWinners: z.number().int().min(1).max(500),
  giveawayMaxDays: z.number().int().min(1).max(365),
  creditGranters: z.array(z.string().regex(/^\d{15,20}$/)).max(20),
  creditMaxPerGrant: z.number().int().min(1).max(1_000_000),
  warnTimeoutAt: z.number().int().min(0).max(100),
  warnTimeout: ruxDuration,
  warnBanAt: z.number().int().min(0).max(100),
  warnBan: ruxDuration,
  ruleCooldownSeconds: z.number().int().min(0).max(86400),
  watchGuidelines: z.boolean(),
  spamEnabled: z.boolean(),
  spamFloodMessages: z.number().int().min(2).max(100),
  spamFloodSeconds: z.number().int().min(1).max(600),
  spamDuplicates: z.number().int().min(2).max(100),
  spamDuplicateSeconds: z.number().int().min(1).max(3600),
  spamMentions: z.number().int().min(2).max(100),
  spamEmoji: z.number().int().min(2).max(500),
  messagesPerMinute: z.number().int().min(1).max(120),
  maxSteps: z.number().int().min(2).max(30),
  historyMinutes: z.number().int().min(1).max(1440),
  // noprofit.space's secrets are 32 characters at least
  noprofitSecret: z.union([z.literal(''), z.string().min(32).max(255)]),
});

// plugged.host: the rux section as the panel sends it (a stored secret comes back as a placeholder);
// its texts are loose, so that one the form's patterns would refuse never breaks loading the settings
export const adminSettingsRuxStoreSchema = adminSettingsRuxSchema.extend({
  name: z.string(),
  guildId: z.string(),
  giveawayChannelId: z.string(),
  modLogChannelId: z.string(),
  trustedDomains: z.array(z.string()),
  servers: z.array(
    z.object({
      id: z.string(),
      name: z.string(),
      about: z.string(),
      giveawayChannelId: z.string(),
      modLogChannelId: z.string(),
      trustedDomains: z.array(z.string()),
    }),
  ),
  timezone: z.string(),
  locale: z.string(),
  creditGranters: z.array(z.string()),
  warnTimeout: z.string(),
  warnBan: z.string(),
  noprofitSecret: z.string().nullable(),
});

// plugged.host: VS Code devboxes next to game servers (shared/src/settings/devboxes.rs,
// docs/features/devboxes.md); an empty token keeps the stored one
export const adminSettingsDevboxesSchema = z.object({
  enabled: z.boolean(),
  eggUuid: z.string().nullable(),
  image: z.string().max(255),
  membersOnly: z.boolean(),
  cpu: z.number().int().min(25).max(3200),
  memory: z.number().int().min(256).max(65536),
  disk: z.number().int().min(512).max(262144),
  premiumCpu: z.number().int().min(25).max(3200),
  premiumMemory: z.number().int().min(256).max(65536),
  premiumDisk: z.number().int().min(512).max(262144),
  idleMinutes: z.number().int().min(1).max(1440),
  maxHours: z.number().int().min(1).max(168),
  runningPerUser: z.number().int().min(1).max(20),
  cleanupDays: z.number().int().min(1).max(365),
  panelUrl: z.string().max(255),
  webDomain: z.string().max(253),
  webOriginIp: z.string().max(45),
  cloudflareToken: z.string().max(255),
});

// plugged.host: the devboxes section as the panel sends it (stored secrets come back as placeholders)
export const adminSettingsDevboxesStoreSchema = adminSettingsDevboxesSchema.extend({
  cloudflareToken: z.string().max(255).nullable(),
  caCert: z.string().nullable(),
  edgeCert: z.string().nullable(),
});

// plugged.host: neo (shared/src/settings/neo.rs, docs/features/neo.md), Admin › Settings › AI › neo; an
// empty contact names none
const neoOptionalEmail = z
  .string()
  .trim()
  .max(255)
  .refine(
    (email) => email === '' || z.email().safeParse(email).success,
    ruleMessage((t) => t('pages.admin.settings.tabs.neo.page.form.rule.email', {})),
  );

// plugged.host: one model slot as the neo tab edits it: the key as typed (empty keeps a stored one),
// extra headers and fields as JSON text
export const adminSettingsNeoProviderFormSchema = z
  .object({
    slot: z.number().int().min(1).max(NEO_MAX_PROVIDERS),
    enabled: z.boolean(),
    label: z.string().trim().max(64),
    description: z.string().trim().max(255),
    driver: z.enum(['openai', 'anthropic', 'gemini']),
    endpoint: z
      .string()
      .trim()
      .max(255)
      .refine(
        (url) => url === '' || /^https:\/\/[^\s/?#]+/i.test(url),
        ruleMessage((t) => t('pages.admin.settings.tabs.neo.page.form.rule.endpoint', {})),
      ),
    apiKey: z.string().max(1024),
    keyStored: z.boolean(),
    clearApiKey: z.boolean(),
    model: z.string().trim().max(128),
    maxTokens: z.number().int().min(256).max(128000),
    timeout: z.number().int().min(5).max(300),
    dailyTokenCap: z.number().int().min(0),
    access: z.enum(['everyone', 'plus', 'plusplus']),
    multiplier: z.number().int().min(1).max(10),
    vision: z.enum(['auto', 'yes', 'no']),
    extraHeaders: z
      .string()
      .max(4096)
      .refine(
        (text) => parseJsonHeaders(text) !== null,
        ruleMessage((t) => t('pages.admin.settings.tabs.neo.page.form.rule.headers', {})),
      ),
    extraBody: z
      .string()
      .max(8192)
      .refine(
        (text) => parseJsonObject(text) !== null,
        ruleMessage((t) => t('pages.admin.settings.tabs.neo.page.form.rule.json', {})),
      ),
  })
  .superRefine((provider, ctx) => {
    // a slot that is off may stay unfinished
    if (!provider.enabled) {
      return;
    }
    for (const field of ['label', 'endpoint', 'model'] as const) {
      if (!provider[field].trim()) {
        ctx.addIssue({
          code: 'custom',
          path: [field],
          message: getTranslations().t('pages.admin.settings.tabs.neo.page.form.rule.required', {}),
        });
      }
    }
  });

export const adminSettingsNeoSchema = z.object({
  enabled: z.boolean(),
  maxSteps: z.number().int().min(1).max(100),
  contextTokens: z.number().int().min(4000).max(400000),
  messagesPerMinute: z.number().int().min(1).max(600),
  toolsPerMinute: z.number().int().min(1).max(600),
  retentionDays: z.number().int().min(1).max(3650),
  supportUrl: z
    .string()
    .trim()
    .max(255)
    .refine(
      (url) => url === '' || /^https?:\/\/\S+$/i.test(url),
      ruleMessage((t) => t('pages.admin.settings.tabs.neo.page.form.rule.supportUrl', {})),
    ),
  legalEmail: neoOptionalEmail,
  privacyEmail: neoOptionalEmail,
  providerDisclosure: z.string().trim().max(255),
  alwaysConfirmDestructive: z.boolean(),
  providers: z.array(adminSettingsNeoProviderFormSchema).max(NEO_MAX_PROVIDERS),
  gatewayEnabled: z.boolean(),
  plusPercent: z.number().int().min(100).max(1000),
  plusplusPercent: z.number().int().min(100).max(1000),
  discordUnlinkedPercent: z.number().int().min(0).max(100),
  shortHours: z.number().int().min(1).max(168),
  shortTokens: z.number().int().min(0),
  longDays: z.number().int().min(1).max(90),
  longTokens: z.number().int().min(0),
  defaultCooldown: z.number().int().min(1).max(3600),
  maxBusyWait: z.number().int().min(1).max(600),
  deadline: z.number().int().min(10).max(300),
  maxConcurrent: z.number().int().min(1).max(64),
  toolsProbe: z.boolean(),
});

// plugged.host: the neo section as the panel sends it (a stored key comes back as "", secret header
// values as the mask), in plain types so a value the forms would refuse never stops the settings loading
export const adminSettingsNeoProviderSchema = z.object({
  slot: z.number(),
  enabled: z.boolean(),
  label: z.string(),
  description: z.string(),
  driver: z.enum(['openai', 'anthropic', 'gemini']),
  endpoint: z.string(),
  apiKey: z.string().nullable(),
  model: z.string(),
  maxTokens: z.number(),
  timeout: z.number(),
  dailyTokenCap: z.number(),
  extraHeaders: z.record(z.string(), z.string()),
  extraBody: z.record(z.string(), z.unknown()),
  access: z.enum(['everyone', 'plus', 'plusplus']),
  multiplier: z.number(),
  vision: z.boolean().nullable(),
});

export const adminSettingsNeoStoreSchema = z.object({
  enabled: z.boolean(),
  maxSteps: z.number(),
  contextTokens: z.number(),
  messagesPerMinute: z.number(),
  toolsPerMinute: z.number(),
  alwaysConfirmDestructive: z.boolean(),
  retentionDays: z.number(),
  supportUrl: z.string(),
  legalEmail: z.string(),
  privacyEmail: z.string(),
  providerDisclosure: z.string(),
  plusPercent: z.number(),
  plusplusPercent: z.number(),
  gatewayEnabled: z.boolean(),
  shortHours: z.number(),
  shortTokens: z.number(),
  longDays: z.number(),
  longTokens: z.number(),
  discordUnlinkedPercent: z.number(),
  defaultCooldown: z.number(),
  maxBusyWait: z.number(),
  deadline: z.number(),
  maxConcurrent: z.number(),
  toolsProbe: z.boolean(),
  providers: z.array(adminSettingsNeoProviderSchema),
});

// plugged.host: the automatic deployment pause (shared/src/settings/placement.rs, docs/features/placement.md)
export const adminSettingsPlacementSchema = z.object({
  pauseEnabled: z.boolean(),
  pausePercent: z.number().int().min(10).max(100),
  smallOffset: z.number().int().min(0).max(50),
  smallMemoryGb: z.number().int().min(0).max(4096),
  smallThreads: z.number().int().min(0).max(1024),
  unreachableAfter: z.number().int().min(1).max(60),
  diskPercent: z.number().int().min(0).max(100),
  // ranges and single ports from 1024 up, comma separated (shared/src/allocations.rs)
  portRanges: z
    .string()
    .trim()
    .regex(/^\d{4,5}(-\d{4,5})?(\s*,\s*\d{4,5}(-\d{4,5})?)*$/),
});

// plugged.host: the start queue (shared/src/settings/queue.rs, docs/features/start-queue.md)
export const adminSettingsQueueSchema = z.object({
  enabled: z.boolean(),
  includeRestart: z.boolean(),
  linePercent: z.number().int().min(1).max(100),
  minWait: z.number().int().min(0).max(300),
  maxWait: z.number().int().min(30).max(86400),
  releaseInterval: z.number().int().min(0).max(3600),
  passMinutes: z.number().int().min(0).max(1440),
  exemptRoles: z.array(z.string()).max(100),
  moveOffer: z.boolean(),
  moveRequiresMembership: z.boolean(),
  moveMaxDisk: z.number().int().min(0).max(1048576),
});

// plugged.host: idle servers stopped and parked (shared/src/settings/limbo.rs, docs/features/limbo.md)
export const adminSettingsLimboSchema = z.object({
  enabled: z.boolean(),
  stopMinutes: z.number().int().min(1).max(525_600),
  parkMinutes: z.number().int().min(1).max(525_600),
  maxPerRun: z.number().int().min(1).max(100),
  costEarly: z.number().int().min(0).max(100000),
  costLate: z.number().int().min(0).max(100000),
  costWindowHours: z.number().int().min(0).max(8760),
  exemptMembers: z.boolean(),
  deleteAfterDays: z.number().int().min(0).max(3650),
});

// plugged.host: ads, the waiting room, the ad-block notice, in-game messages (shared/src/settings/ads.rs, docs/features/ads.md)
export const adminSettingsAdsSchema = z.object({
  enabled: z.boolean(),
  showAccount: z.boolean(),
  showServer: z.boolean(),
  memberFree: z.boolean(),
  slotTop: z.boolean(),
  slotServerTools: z.boolean(),
  slotPages: z.boolean(),
  slotFoot: z.boolean(),
  maxUnitsPerPage: z.number().int().min(0).max(20),
  refreshSeconds: z.number().int().min(0).max(3600),
  refreshMemberSeconds: z.number().int().min(0).max(3600),
  zones: z.string().max(20000),
  frameOrigin: z.string().trim().max(255),
  nativeScript: z.string().max(2000),
  nativeContainer: z.string().max(128),
  nativeHeight: z.number().int().min(50).max(2000),
  socialBarScript: z.string().max(2000),
  socialBarEverywhere: z.boolean(),
  popunderScript: z.string().max(2000),
  roomScript: z.string().max(2000),
  monetagPushZone: z.string().trim().regex(/^\d*$/).max(12),
  monetagPushSrc: z.string().max(2000),
  monetagVignetteZone: z.string().trim().regex(/^\d*$/).max(12),
  monetagVignetteSrc: z.string().max(2000),
  monetagVignetteMinutes: z.number().int().min(0).max(1440),
  monetagVignetteMembers: z.boolean(),
  monetagVerification: z.string().trim().max(128),
  cspHosts: z.string().max(4000),
  roomEnabled: z.boolean(),
  roomCpu: z.number().int().min(0).max(1_000_000),
  roomMemory: z.number().int().min(0).max(100_000_000),
  roomDisk: z.number().int().min(0).max(100_000_000),
  roomMinSeconds: z.number().int().min(0).max(600),
  roomMaxSeconds: z.number().int().min(1).max(600),
  roomPassMinutes: z.number().int().min(0).max(1440),
  roomBanner: z.boolean(),
  roomNative: z.boolean(),
  roomSocialBar: z.boolean(),
  roomPopunder: z.boolean(),
  roomInterstitial: z.boolean(),
  adblockEnabled: z.boolean(),
  adblockMode: z.enum(['notice', 'modal', 'block']),
  adblockGrace: z.number().int().min(0).max(60),
  adblockDismissHours: z.number().int().min(0).max(720),
  adblockProbe: z.boolean(),
  adblockSkipConsole: z.boolean(),
  adblockMessage: z.string().max(1000),
  inGameEnabled: z.boolean(),
  inGameMessage: z.string().max(200),
  inGameInterval: z.number().int().min(5).max(1440),
});

// plugged.host: deleted servers' copies and node failover (shared/src/settings/recovery.rs, docs/features/backups-page.md, failover.md)
export const adminSettingsRecoverySchema = z.object({
  deletedHoldDays: z.number().int().min(0).max(90),
  failoverMode: z.enum(['off', 'manual', 'notify', 'auto']),
  failoverAfter: z.number().int().min(3).max(1440),
  failoverParallel: z.number().int().min(1).max(20),
});

// plugged.host: credits, tokens, memberships, the Resource Store (shared/src/settings/economy.rs, docs/features/economy.md)
export const adminSettingsEconomySchema = z.object({
  creditsEnabled: z.boolean(),
  startingCredits: z.number().int().min(0).max(1_000_000),
  tokensEnabled: z.boolean(),
  dailyTokens: z.number().int().min(0).max(100),
  panelAccessEnabled: z.boolean(),
  panelAccessTokens: z.number().int().min(0).max(100_000),
  panelAccessDays: z.number().int().min(1).max(3650),
  membershipEnabled: z.boolean(),
  plusName: z.string().trim().min(1).max(32),
  plusplusName: z.string().trim().min(1).max(32),
  plusTokens: z.number().int().min(0).max(100_000),
  plusDays: z.number().int().min(1).max(365),
  plusplusTokens: z.number().int().min(0).max(100_000),
  plusplusTokenDays: z.number().int().min(1).max(365),
  plusplusCredits: z.number().int().min(0).max(1_000_000),
  plusplusCreditDays: z.number().int().min(1).max(365),
  plusBurstCores: z.number().int().min(0).max(16),
  plusplusBurstCores: z.number().int().min(0).max(16),
  cpuStep: z.number().int().min(0).max(10_000),
  cpuPrice: z.number().int().min(0).max(1_000_000),
  memoryStep: z.number().int().min(0).max(1_048_576),
  memoryPrice: z.number().int().min(0).max(1_000_000),
  diskStep: z.number().int().min(0).max(10_485_760),
  diskPrice: z.number().int().min(0).max(1_000_000),
  limitCpu: z.number().int().min(1).max(100_000),
  limitMemory: z.number().int().min(1).max(10_485_760),
  limitDisk: z.number().int().min(1).max(104_857_600),
  priceFloor: z.number().int().min(0).max(100),
  softLimit: z.number().int().min(100).max(1000),
  softMultiplier: z.number().int().min(1).max(100),
  doubling: z.number().int().min(1).max(100),
  storeDiscount: z.number().int().min(0).max(100),
  storeDiscountStarts: z.coerce.date().nullable(),
  storeDiscountEnds: z.coerce.date().nullable(),
  storeDiscountName: z.preprocess(nullableString, z.string().max(64).nullable()),
  claimMinutes: z.number().int().min(1).max(1440),
  // plugged.host: each further link of the day pays less (shared/src/earn.rs link_reward)
  linkDecayPercent: z.number().int().min(0).max(90),
  linkFloorPercent: z.number().int().min(1).max(100),
  linkvertiseEnabled: z.boolean(),
  linkvertisePublisher: z.preprocess(
    nullableString,
    z
      .string()
      .regex(/^[0-9]+$/)
      .max(20)
      .nullable(),
  ),
  linkvertiseWhitelist: z.preprocess(nullableString, z.string().max(255).nullable()),
  linkvertiseLink: z.preprocess(
    nullableString,
    z
      .url({ protocol: /^https$/ })
      .max(1024)
      .nullable(),
  ),
  linkvertiseToken: z.string().max(512).nullable(),
  linkvertiseReward: z.number().min(0.01).max(1000), // plugged.host: up to 2 decimals
  linkvertiseDailyCap: z.number().int().min(0).max(1000),
  vyloEnabled: z.boolean(),
  vyloApiKey: z.string().max(512).nullable(),
  vyloReward: z.number().min(0.01).max(1000), // plugged.host: up to 2 decimals
  vyloDailyCap: z.number().int().min(0).max(1000),
  vyloMinSeconds: z.number().int().min(0).max(600),
  cutyEnabled: z.boolean(),
  cutyApiKey: z.string().max(512).nullable(),
  cutyReward: z.number().min(0.01).max(1000), // plugged.host: up to 2 decimals
  cutyDailyCap: z.number().int().min(0).max(1000),
  cutyMinSeconds: z.number().int().min(0).max(600),
  shrinkearnEnabled: z.boolean(),
  shrinkearnApiKey: z.string().max(512).nullable(),
  shrinkearnReward: z.number().min(0.01).max(1000), // plugged.host: up to 2 decimals
  shrinkearnDailyCap: z.number().int().min(0).max(1000),
  shrinkearnMinSeconds: z.number().int().min(0).max(600),
  shrinkmeEnabled: z.boolean(),
  shrinkmeApiKey: z.string().max(512).nullable(),
  shrinkmeReward: z.number().min(0.01).max(1000), // plugged.host: up to 2 decimals
  shrinkmeDailyCap: z.number().int().min(0).max(1000),
  shrinkmeMinSeconds: z.number().int().min(0).max(600),
  adfocusEnabled: z.boolean(),
  adfocusApiKey: z.string().max(512).nullable(),
  adfocusReward: z.number().min(0.01).max(1000), // plugged.host: up to 2 decimals
  adfocusDailyCap: z.number().int().min(0).max(1000),
  adfocusMinSeconds: z.number().int().min(0).max(600),
  // plugged.host: CPX Research's survey wall (shared/src/earn.rs)
  cpxResearchEnabled: z.boolean(),
  cpxResearchAppId: z.preprocess(
    nullableString,
    z
      .string()
      .regex(/^[0-9]+$/)
      .max(20)
      .nullable(),
  ),
  cpxResearchSecret: z.string().max(512).nullable(),
  cpxResearchDailyCap: z.number().int().min(0).max(1000),
});

// plugged.host: the welcome tour (shared/src/settings/tour.rs, docs/features/welcome-tour.md)
export const adminSettingsTourSchema = z.object({
  enabled: z.boolean(),
  version: z.number().int().min(1).max(1000),
});

// plugged.host: asking players for a review (shared/src/settings/reviews.rs, docs/features/reviews.md)
export const adminSettingsReviewsSchema = z.object({
  url: z.union([z.literal(''), z.url({ protocol: /^https$/ }).max(512)]),
  site: z.string().max(64),
  delayDays: z.number().int().min(0).max(365),
  intervalDays: z.number().int().min(0).max(3650),
});

// plugged.host: the partner program (shared/src/settings/partners.rs, docs/features/partners.md)
export const adminSettingsPartnersSchema = z
  .object({
    enabled: z.boolean(),
    minMembers: z.number().int().min(1).max(10_000_000),
    minCredits: z.number().int().min(0).max(10_000_000),
    maxCredits: z.number().int().min(0).max(10_000_000),
    cooldownDays: z.number().int().min(0).max(365),
    graceDays: z.number().int().min(0).max(60),
    channelRequired: z.boolean(),
    channelNames: z.string().max(500),
    channelLinks: z.string().max(500),
    neoAssessment: z.boolean(),
    guidance: z.string().max(2000),
  })
  .refine((values) => values.minCredits <= values.maxCredits, { path: ['minCredits'] });

// plugged.host: neo on Discord (shared/src/settings/neo_discord.rs, docs/features/neo-discord.md)
export const adminSettingsNeoDiscordSchema = z.object({
  nameCalls: z.boolean(),
  messagesPerMinute: z.number().int().min(1).max(60),
  maxSteps: z.number().int().min(2).max(20),
  historyMinutes: z.number().int().min(1).max(1440),
  historyTurns: z.number().int().min(0).max(20),
});

export const adminSettingsUpdatesSchema = z.object({
  wingsReleaseToken: z.preprocess(nullableString, z.string().min(1).max(1024).nullable()),
  dbAgentReleaseToken: z.preprocess(nullableString, z.string().min(1).max(1024).nullable()),
});

export const twoFactorRequirement = z.enum(['admins', 'all_users', 'none']);

export const adminSettingsSchema = z.object({
  oobeStep: oobeStepKey.nullable(),
  telemetryUuid: z.string().uuid().nullable().optional(),
  telemetryCronSchedule: z.string().nullable().optional(),
  storageDriver: adminSettingsStorageSchema,
  mailMode: adminSettingsEmailSchema,
  captchaProvider: adminSettingsCaptchaProviderSchema,
  app: adminSettingsApplicationSchema,
  metadata: adminSettingsMetadataSchema,
  webauthn: adminSettingsWebauthnSchema,
  server: adminSettingsServerSchema,
  user: adminSettingsUserSchema,
  activity: adminSettingsActivitySchema,
  ratelimits: adminSettingsRatelimitsSchema,
  noprofit: adminSettingsNoprofitSchema, // plugged.host
  status: adminSettingsStatusSchema, // plugged.host
  abuse: adminSettingsAbuseSchema, // plugged.host
  games: adminSettingsGamesStoreSchema, // plugged.host
  placement: adminSettingsPlacementSchema, // plugged.host
  tour: adminSettingsTourSchema, // plugged.host
  reviews: adminSettingsReviewsSchema, // plugged.host
  partners: adminSettingsPartnersSchema, // plugged.host
  recovery: adminSettingsRecoverySchema, // plugged.host
  economy: adminSettingsEconomySchema, // plugged.host
  queue: adminSettingsQueueSchema, // plugged.host
  limbo: adminSettingsLimboSchema, // plugged.host
  ads: adminSettingsAdsSchema, // plugged.host
  subdomains: adminSettingsSubdomainsStoreSchema, // plugged.host
  discord: adminSettingsDiscordStoreSchema, // plugged.host
  rux: adminSettingsRuxStoreSchema, // plugged.host
  devboxes: adminSettingsDevboxesStoreSchema, // plugged.host
  neo: adminSettingsNeoStoreSchema, // plugged.host
  neoDiscord: adminSettingsNeoDiscordSchema, // plugged.host
  updates: adminSettingsUpdatesSchema,
});
