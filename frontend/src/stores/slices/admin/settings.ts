import { z } from 'zod';
import { StateCreator } from 'zustand';
import getUpdates from '@/api/admin/system/updates/getUpdates.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import { adminSettingsSchema } from '@/lib/schemas/admin/settings.ts';
import { adminUpdateInformationSchema } from '@/lib/schemas/admin/system.ts';
import { AdminStore } from '@/stores/admin.tsx';

export interface SettingsSlice extends z.infer<typeof adminSettingsSchema> {
  updateInformation: z.infer<typeof adminUpdateInformationSchema> | null;
  updateInformationError: string | null;

  setSettings: (settings: z.infer<typeof adminSettingsSchema>) => void;
  updateSettings: (settings: Partial<z.infer<typeof adminSettingsSchema>>) => void;
  setUpdateInformation: (updateInformation: z.infer<typeof adminUpdateInformationSchema> | null) => void;
  loadUpdateInformation: () => Promise<void>;
}

export const createSettingsSlice: StateCreator<AdminStore, [], [], SettingsSlice> = (set): SettingsSlice => ({
  oobeStep: null,
  storageDriver: {
    type: 'filesystem',
    path: '',
  },
  mailMode: {
    type: 'none',
  },
  captchaProvider: {
    type: 'none',
  },
  app: {
    name: '',
    icon: '',
    iconLight: null,
    language: '',
    url: '',
    additionalUrls: [],
    banner: null,
    bannerLight: null,
    twoFactorRequirement: 'none',
    emailTwoFactorEnabled: false,
    twoFactorAcceptedMethods: ['totp', 'security_key'],
    emailVerificationRequired: false,
    sessionCookie: '',
    sessionDurationSeconds: 3600,
    telemetryEnabled: true,
    registrationEnabled: true,
    passwordLoginEnabled: true,
  },
  metadata: {
    description: null,
    // plugged.host: Theme v2's accent, its blue by default (lib/accent.ts).
    themeColor: '#1d4ed8',
    ogImage: null,
    twitterCard: 'summary_large_image',
    indexable: true,
  },
  server: {
    maxFileManagerViewSize: 10 * 1024 * 1024 * 1024,
    maxFileManagerContentSearchSize: 5 * 1024 * 1024 * 1024,
    maxFileManagerSearchResults: 100,
    maxSubuserCount: 25,
    maxScheduleStepCount: 50,
    maxBackupGroupCount: 10,
    maxFirewallRuleCount: 100,
    maxFirewallRuleSourceCount: 50,
    maxTunnelConnectionCount: 25,
    maxTunnelPortCount: 10,
    maxDatabaseInstanceDatabaseCount: 10,
    maxDatabaseInstanceUserCount: 10,
    allowOverwritingCustomDockerImage: true,
    allowViewingInstallationLogs: true,
    allowAcknowledgingInstallationFailure: true,
    allowViewingTransferProgress: true,
    trash: {
      retention: 7 * 24 * 60 * 60,
      usageThreshold: 90,
    },
    allowChangingTrashSettings: true,
    containerPrelude: '\x1b[1m\x1b[33mcontainer@calagopus~ \x1b[0m',
  },
  user: {
    maxServerGroupCount: 25,
    maxApiKeyCount: 50,
    maxCommandSnippetCount: 100,
    maxSecurityKeyCount: 50,
    maxSshKeyCount: 50,
    maxSettingsCount: 512,
    maxSettingsValueBytes: 16384,
    allowChangingLanguage: true,
    allowChangingAvatar: true,
    routeOrder: null,
  },
  webauthn: {
    enabled: true,
    allowDiscoverable: true,
    rpId: '',
    rpOrigin: '',
    authenticationTimeoutSeconds: 300,
    registrationTimeoutSeconds: 300,
  },
  activity: {
    adminLogRetentionDays: 180,
    adminLogRetentionCount: null,
    userLogRetentionDays: 90,
    userLogRetentionCount: null,
    serverLogRetentionDays: 90,
    serverLogRetentionCount: null,
    serverLogAdminActivity: true,
    serverLogScheduleActivity: true,
    serverHideActivityIps: 'none',
  },
  ratelimits: {
    authRegister: {
      hits: 0,
      windowSeconds: 0,
    },
    authLogin: {
      hits: 0,
      windowSeconds: 0,
    },
    authLoginCheckpoint: {
      hits: 0,
      windowSeconds: 0,
    },
    authLoginCheckpointEmail: {
      hits: 0,
      windowSeconds: 0,
    },
    authLoginSecurityKey: {
      hits: 0,
      windowSeconds: 0,
    },
    authPasswordForgot: {
      hits: 0,
      windowSeconds: 0,
    },
    authPasswordReset: {
      hits: 0,
      windowSeconds: 0,
    },
    authEmailVerification: {
      hits: 0,
      windowSeconds: 0,
    },
    client: {
      hits: 0,
      windowSeconds: 0,
    },
    clientAccountEmailResendVerification: {
      hits: 0,
      windowSeconds: 0,
    },
    clientServersBackupsCreate: {
      hits: 0,
      windowSeconds: 0,
    },
    clientServersFilesPull: {
      hits: 0,
      windowSeconds: 0,
    },
    clientServersFilesPullQuery: {
      hits: 0,
      windowSeconds: 0,
    },
    remote: {
      hits: 0,
      windowSeconds: 0,
    },
    remoteSftpAuth: {
      hits: 0,
      windowSeconds: 0,
    },
    remoteEnroll: {
      hits: 0,
      windowSeconds: 0,
    },
    exemptIps: [],
    exemptApiKeys: [],
  },
  // plugged.host: noprofit.space
  noprofit: {
    enabled: false,
    accountsUrl: 'https://accounts.noprofit.space',
    ssoClient: '',
    ssoAudience: null,
    ssoCallbackUrl: null,
    adminsOnly: false,
    linkByEmail: true,
    ssoSecret: null,
    apiSecret: null,
  },
  // plugged.host: the status page
  status: {
    enabled: false,
    siteUrl: null,
    siteHost: null,
    panelUrl: null,
    pageUrl: null,
    historyDays: 90,
    cpuThreshold: 95,
  },
  // plugged.host: abuse protection
  abuse: {
    mode: 'report',
    alertWebhook: null,
    cpuPercent: 90,
    cpuMinutes: 20,
    deleteAfterHours: 24,
  },
  // plugged.host: what players may create, the deployment pause
  games: {
    createEnabled: false,
    catalog: [],
    limitAllocations: 1,
    limitDatabases: 0,
    limitBackups: 0,
    limitSchedules: 0,
    allowResizing: false,
    allowDeleting: false,
    transferEnabled: false,
    transferCooldownHours: 24,
    transferAttempts: 2,
    transferParallel: 2,
    maxServers: 0,
    allowanceCpu: 0,
    allowanceMemory: 0,
    allowanceDisk: 0,
  },
  placement: {
    pauseEnabled: true,
    pausePercent: 85,
    smallOffset: 10,
    smallMemoryGb: 34,
    smallThreads: 9,
    unreachableAfter: 3,
  },
  tour: {
    enabled: false,
    version: 1,
  },
  recovery: {
    deletedHoldDays: 7,
    failoverMode: 'manual',
    failoverAfter: 10,
    failoverParallel: 3,
  },
  updates: {
    wingsReleaseToken: null,
    dbAgentReleaseToken: null,
  },

  updateInformation: null,
  updateInformationError: null,

  setSettings: (value) =>
    set(() => ({
      storageDriver: value.storageDriver,
      mailMode: value.mailMode,
      captchaProvider: value.captchaProvider,
      app: value.app,
      metadata: value.metadata,
      server: value.server,
      user: value.user,
      webauthn: value.webauthn,
      activity: value.activity,
      ratelimits: value.ratelimits,
      noprofit: value.noprofit, // plugged.host
      status: value.status, // plugged.host
      abuse: value.abuse, // plugged.host
      games: value.games, // plugged.host
      placement: value.placement, // plugged.host
      tour: value.tour, // plugged.host
      recovery: value.recovery, // plugged.host
      updates: value.updates,
    })),
  updateSettings: (value) =>
    set((state) => ({
      ...(value.storageDriver ? { storageDriver: value.storageDriver } : {}),
      ...(value.mailMode ? { mailMode: value.mailMode } : {}),
      ...(value.captchaProvider ? { captchaProvider: value.captchaProvider } : {}),
      ...(value.app ? { app: { ...state.app, ...value.app } } : {}),
      ...(value.metadata ? { metadata: { ...state.metadata, ...value.metadata } } : {}),
      ...(value.server ? { server: { ...state.server, ...value.server } } : {}),
      ...(value.user ? { user: { ...state.user, ...value.user } } : {}),
      ...(value.webauthn ? { webauthn: { ...state.webauthn, ...value.webauthn } } : {}),
      ...(value.activity ? { activity: { ...state.activity, ...value.activity } } : {}),
      ...(value.ratelimits ? { ratelimits: { ...state.ratelimits, ...value.ratelimits } } : {}),
      ...(value.noprofit ? { noprofit: { ...state.noprofit, ...value.noprofit } } : {}), // plugged.host
      ...(value.status ? { status: { ...state.status, ...value.status } } : {}), // plugged.host
      ...(value.abuse ? { abuse: { ...state.abuse, ...value.abuse } } : {}), // plugged.host
      ...(value.games ? { games: { ...state.games, ...value.games } } : {}), // plugged.host
      ...(value.placement ? { placement: { ...state.placement, ...value.placement } } : {}), // plugged.host
      ...(value.tour ? { tour: { ...state.tour, ...value.tour } } : {}), // plugged.host
      ...(value.recovery ? { recovery: { ...state.recovery, ...value.recovery } } : {}), // plugged.host
      ...(value.updates ? { updates: { ...state.updates, ...value.updates } } : {}),
    })),
  setUpdateInformation: (value) => set((state) => ({ ...state, updateInformation: value })),
  loadUpdateInformation: async () => {
    set((state) => ({ ...state, updateInformationError: null }));

    try {
      const updateInformation = await getUpdates();
      set((state) => ({ ...state, updateInformation }));
    } catch (error) {
      set((state) => ({ ...state, updateInformationError: httpErrorToHuman(error) }));
    }
  },
});
