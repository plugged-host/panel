// plugged.host: Admin › Settings › AI › neo, the section as the panel keeps it and as the form edits it
import { z } from 'zod';
import { formatJsonObject } from '@/lib/neo.ts';
import {
  adminSettingsNeoProviderSchema,
  adminSettingsNeoSchema,
  adminSettingsNeoStoreSchema,
} from '@/lib/schemas/admin/settings.ts';

export type NeoValues = z.infer<typeof adminSettingsNeoSchema>;
export type NeoProviderValues = NeoValues['providers'][number];

export const neoEmptyFormValues: NeoValues = {
  enabled: false,
  maxSteps: 25,
  contextTokens: 24000,
  messagesPerMinute: 10,
  toolsPerMinute: 60,
  retentionDays: 30,
  supportUrl: '',
  legalEmail: '',
  privacyEmail: '',
  providerDisclosure: '',
  alwaysConfirmDestructive: true,
  providers: [],
  gatewayEnabled: true,
  plusPercent: 125,
  plusplusPercent: 125,
  discordUnlinkedPercent: 50,
  shortHours: 5,
  shortTokens: 3_000_000,
  longDays: 7,
  longTokens: 15_000_000,
  defaultCooldown: 60,
  maxBusyWait: 60,
  deadline: 80,
  maxConcurrent: 4,
  toolsProbe: true,
};

// The key never comes back: the field stays empty and keeps it.
export const neoProviderToFormValues = (
  provider: z.infer<typeof adminSettingsNeoProviderSchema>,
): NeoProviderValues => ({
  slot: provider.slot,
  enabled: provider.enabled,
  label: provider.label,
  description: provider.description,
  driver: provider.driver,
  endpoint: provider.endpoint,
  apiKey: '',
  keyStored: provider.apiKey !== null,
  clearApiKey: false,
  model: provider.model,
  maxTokens: provider.maxTokens,
  timeout: provider.timeout,
  dailyTokenCap: provider.dailyTokenCap,
  access: provider.access,
  multiplier: provider.multiplier,
  vision: provider.vision === null ? 'auto' : provider.vision ? 'yes' : 'no',
  extraHeaders: formatJsonObject(provider.extraHeaders),
  extraBody: formatJsonObject(provider.extraBody),
});

export const neoToFormValues = (neo: z.infer<typeof adminSettingsNeoStoreSchema>): NeoValues => ({
  ...neo,
  providers: [...neo.providers].sort((a, b) => a.slot - b.slot).map(neoProviderToFormValues),
});

export const newNeoProvider = (slot: number): NeoProviderValues => ({
  slot,
  enabled: true,
  label: '',
  description: '',
  driver: 'openai',
  endpoint: '',
  apiKey: '',
  keyStored: false,
  clearApiKey: false,
  model: '',
  maxTokens: 4096,
  timeout: 30,
  dailyTokenCap: 0,
  access: 'everyone',
  multiplier: 1,
  vision: 'auto',
  extraHeaders: '',
  extraBody: '',
});
