import { z } from 'zod';
import { eggConfigurationRouteItemSchema } from '@/lib/schemas/generic.ts';
import { oobeStepKey } from '@/lib/schemas/oobe.ts';
import { twoFactorMethod } from '@/lib/schemas/user.ts';

export const publicSettingsCaptchaProviderNoneSchema = z.object({
  type: z.literal('none'),
});

export const publicSettingsCaptchaProviderTurnstileSchema = z.object({
  type: z.literal('turnstile'),
  siteKey: z.string(),
});

export const publicSettingsCaptchaProviderRecaptchaSchema = z.object({
  type: z.literal('recaptcha'),
  siteKey: z.string(),
  v3: z.boolean(),
});

export const publicSettingsCaptchaProviderHcaptchaSchema = z.object({
  type: z.literal('hcaptcha'),
  siteKey: z.string(),
});

export const publicSettingsCaptchaProviderFriendlyCaptchaSchema = z.object({
  type: z.literal('friendly_captcha'),
  siteKey: z.string(),
});

export const publicSettingsCaptchaProviderCapSchema = z.object({
  type: z.literal('cap'),
  apiUrl: z.string(),
  siteKey: z.string(),
});

export const publicSettingsCaptchaProviderSchema = z.discriminatedUnion('type', [
  publicSettingsCaptchaProviderNoneSchema,
  publicSettingsCaptchaProviderTurnstileSchema,
  publicSettingsCaptchaProviderRecaptchaSchema,
  publicSettingsCaptchaProviderHcaptchaSchema,
  publicSettingsCaptchaProviderFriendlyCaptchaSchema,
  publicSettingsCaptchaProviderCapSchema,
]);

// plugged.host: the public part of the ad settings (docs/features/ads.md)
const adsMonetagTagSchema = z.object({ zone: z.string(), src: z.string() });

export const adsZoneSchema = z.object({
  key: z.string(),
  width: z.number(),
  height: z.number(),
  host: z.string(),
});

export const adsSettingsSchema = z.object({
  enabled: z.boolean(),
  showAccount: z.boolean(),
  showServer: z.boolean(),
  slotTop: z.boolean(),
  slotServerTools: z.boolean(),
  slotPages: z.boolean(),
  slotFoot: z.boolean(),
  refreshSeconds: z.number(),
  refreshMemberSeconds: z.number(),
  zones: z.array(adsZoneSchema),
  frameOrigin: z.string().nullable(),
  native: z.boolean(),
  nativeHeight: z.number(),
  socialBarScript: z.string().nullable(),
  socialBarEverywhere: z.boolean(),
  popunderScript: z.string().nullable(),
  roomScript: z.string().nullable(),
  monetagPush: adsMonetagTagSchema.nullable(),
  monetagVignette: adsMonetagTagSchema.nullable(),
  monetagVignetteMinutes: z.number(),
  monetagVignetteMembers: z.boolean(),
  roomBanner: z.boolean(),
  roomNative: z.boolean(),
  roomSocialBar: z.boolean(),
  roomPopunder: z.boolean(),
  roomInterstitial: z.boolean(),
  adblockEnabled: z.boolean(),
  adblockMode: z.enum(['notice', 'modal', 'block']),
  adblockGrace: z.number(),
  adblockDismissHours: z.number(),
  adblockProbe: z.boolean(),
  adblockSkipConsole: z.boolean(),
  adblockMessage: z.string().nullable(),
});

export const publicSettingsSchema = z.object({
  time: z.string(),
  oobeStep: oobeStepKey.nullable(),
  disabledExtensions: z.array(z.string()),
  captchaProvider: publicSettingsCaptchaProviderSchema,
  app: z.object({
    url: z.string(),
    additionalUrls: z.array(z.string()),
    icon: z.string(),
    iconLight: z.string().nullable(),
    banner: z.string().nullable(),
    bannerLight: z.string().nullable(),
    name: z.string(),
    language: z.string(),
    registrationEnabled: z.boolean(),
    passwordLoginEnabled: z.boolean(),
    emailTwoFactorEnabled: z.boolean(),
    emailVerificationRequired: z.boolean(),
    twoFactorAcceptedMethods: z.array(twoFactorMethod),
    debug: z.boolean(),
    release: z.string(),
    // plugged.host: Theme v2's accent (lib/accent.ts).
    themeColor: z.string(),
  }),
  webauthn: z.object({
    enabled: z.boolean(),
    allowDiscoverable: z.boolean(),
  }),
  server: z.object({
    maxFileManagerViewSize: z.number(),
    maxFileManagerContentSearchSize: z.number(),
    maxFileManagerSearchResults: z.number(),
    maxSubuserCount: z.number(),
    maxScheduleStepCount: z.number(),
    maxBackupGroupCount: z.number(),
    maxFirewallRuleCount: z.number(),
    maxFirewallRuleSourceCount: z.number(),
    maxTunnelConnectionCount: z.number(),
    maxTunnelPortCount: z.number(),
    maxDatabaseInstanceDatabaseCount: z.number(),
    maxDatabaseInstanceUserCount: z.number(),
    allowOverwritingCustomDockerImage: z.boolean(),
    allowAcknowledgingInstallationFailure: z.boolean(),
    allowChangingTrashSettings: z.boolean(),
    containerPrelude: z.string(),
  }),
  user: z.object({
    maxServerGroupCount: z.number(),
    maxApiKeyCount: z.number(),
    maxCommandSnippetCount: z.number(),
    maxSecurityKeyCount: z.number(),
    maxSshKeyCount: z.number(),

    allowChangingLanguage: z.boolean(),
    allowChangingAvatar: z.boolean(),

    routeOrder: z.array(eggConfigurationRouteItemSchema).nullable(),
  }),
  // plugged.host: noprofit.space sign-in, when it is set up.
  noprofit: z.object({
    enabled: z.boolean(),
    accountsUrl: z.string().nullable(),
    adminsOnly: z.boolean(),
    sftpPasswords: z.boolean(),
  }),
  // plugged.host: what players may do with servers themselves
  games: z.object({
    createEnabled: z.boolean(),
    allowResizing: z.boolean(),
    allowDeleting: z.boolean(),
    transferEnabled: z.boolean(),
    subdomainsEnabled: z.boolean(),
  }),
  // plugged.host: the welcome tour runs
  tour: z.object({
    enabled: z.boolean(),
  }),
  // plugged.host: which parts of the economy players see (docs/features/economy.md)
  economy: z.object({
    creditsEnabled: z.boolean(),
    tokensEnabled: z.boolean(),
    membershipEnabled: z.boolean(),
    plusName: z.string(),
    plusplusName: z.string(),
  }),
  // plugged.host: what pages need to show ads (docs/features/ads.md)
  ads: adsSettingsSchema,
});
