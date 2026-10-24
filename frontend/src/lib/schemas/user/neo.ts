// plugged.host: neo's chat, its preferences, history and usage (shared/src/neo, docs/features/neo.md)
import { z } from 'zod';

// Why the person cannot chat here right now (Agent::blocked).
export const neoBlockedSchema = z.enum(['disabled', 'turned_off', 'consent', 'no_access', 'not_yours']);

export const neoPreferencesSchema = z.object({
  enabled: z.boolean(),
  autoApprove: z.boolean(),
  model: z.string().nullable(),
  acknowledged: z.string().nullable(),
});

export const neoToolStatusSchema = z.enum([
  'pending_approval',
  'approved',
  'queued',
  'executing',
  'done',
  'failed',
  'denied',
  'skipped',
]);

// values for a notice's text (retry_at, window, model, access): kept as the backend names them
export const neoParamsSchema = z.record(z.string(), z.unknown()).nullable();

export const neoMessageSchema = z.object({
  uuid: z.string(),
  seq: z.number(),
  role: z.enum(['user', 'assistant', 'tool', 'notice']),
  content: z.string().nullable(),
  tool: z.string().nullable(),
  status: neoToolStatusSchema.nullable(),
  summary: z.string().nullable(),
  risk: z.enum(['read', 'write', 'destructive', 'critical']).nullable(),
  approval: z.enum(['auto', 'user', 'denied']).nullable(),
  preview: z.string().nullable(),
  display: z.string().nullable(),
  images: z.array(z.string()),
  thinking: z.string().nullable(),
  model: z.string().nullable(),
  code: z.string().nullable(),
  params: neoParamsSchema,
  created: z.string(),
});

export const neoConversationSchema = z.object({
  uuid: z.string(),
  serverUuid: z.string().nullable(),
  title: z.string().nullable(),
  status: z.enum(['idle', 'running', 'awaiting_approval', 'limited', 'unlinked', 'error']),
  archived: z.boolean(),
  messages: z.array(neoMessageSchema),
  lastActivity: z.string(),
  created: z.string(),
});

export const neoSchema = z.object({
  blocked: neoBlockedSchema.nullable(),
  preferences: neoPreferencesSchema,
  conversation: neoConversationSchema,
  supportUrl: z.string(),
  legalEmail: z.string(),
  privacyEmail: z.string(),
  providerDisclosure: z.string(),
  maxMessageChars: z.number(),
  maxImages: z.number(),
});

// What the chat does next (agent.rs StepResult): continue and busy step again.
export const neoStepResultSchema = z.object({
  state: z.enum([
    'continue',
    'busy',
    'awaiting_approval',
    'done',
    'limited',
    'unlinked',
    'error',
    'blocked',
    'redirect',
  ]),
  code: z.string().nullable(),
  params: neoParamsSchema,
  retryAfter: z.number(),
  url: z.string().nullable(),
});

export const neoStepResponseSchema = z.object({
  result: neoStepResultSchema,
  conversation: neoConversationSchema,
});

// the last line of a streamed step: the conversation is null when it could not be read
export const neoStreamResultSchema = z.object({
  result: neoStepResultSchema,
  conversation: neoConversationSchema.nullable(),
});

export const neoHistoryEntrySchema = z.object({
  uuid: z.string(),
  title: z.string().nullable(),
  archived: z.boolean(),
  userMessages: z.number(),
  lastActivity: z.string(),
});

export const neoWindowSchema = z.object({
  used: z.number(),
  limit: z.number(),
  percent: z.number(),
  active: z.boolean(),
  startedAt: z.string().nullable(),
  resetsAt: z.string().nullable(),
  windowSeconds: z.number(),
});

export const neoModelSchema = z.object({
  key: z.string(),
  label: z.string(),
  description: z.string(),
  // everyone, plus or plusplus
  access: z.string(),
  allowed: z.boolean(),
  limited: z.boolean(),
  multiplier: z.number(),
  vision: z.boolean(),
  thinking: z.boolean(),
  window: neoWindowSchema.nullable(),
});

export const neoUsageSchema = z.object({
  linked: z.boolean(),
  // by kind: short and long, and from noprofit.space possibly more
  limits: z.record(z.string(), neoWindowSchema).nullable(),
  // all (every model in the shared windows) or models (only limited ones)
  mode: z.string(),
  models: z.array(neoModelSchema),
  defaultModel: z.string().nullable(),
});

export const neoPreferencesUpdateSchema = z.object({
  enabled: z.boolean().optional(),
  autoApprove: z.boolean().optional(),
  // '' goes back to the default model
  model: z.string().optional(),
  acknowledged: z.literal(true).optional(),
});

export const neoSeedSchema = z.object({
  tool: z.string(),
  arguments: z.record(z.string(), z.unknown()),
});

export type NeoBlocked = z.infer<typeof neoBlockedSchema>;
export type NeoPreferences = z.infer<typeof neoPreferencesSchema>;
export type NeoMessage = z.infer<typeof neoMessageSchema>;
export type NeoConversation = z.infer<typeof neoConversationSchema>;
export type NeoInfo = z.infer<typeof neoSchema>;
export type NeoStepResult = z.infer<typeof neoStepResultSchema>;
export type NeoStepResponse = z.infer<typeof neoStepResponseSchema>;
export type NeoStreamResult = z.infer<typeof neoStreamResultSchema>;
export type NeoHistoryEntry = z.infer<typeof neoHistoryEntrySchema>;
export type NeoWindow = z.infer<typeof neoWindowSchema>;
export type NeoModel = z.infer<typeof neoModelSchema>;
export type NeoUsage = z.infer<typeof neoUsageSchema>;
export type NeoPreferencesUpdate = z.infer<typeof neoPreferencesUpdateSchema>;
export type NeoSeed = z.infer<typeof neoSeedSchema>;
