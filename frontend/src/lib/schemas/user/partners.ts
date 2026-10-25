// plugged.host: Account › Partners (shared/src/partners.rs, docs/features/partners.md)
import { z } from 'zod';

export const partnerStatus = z.enum(['pending', 'approved', 'paused', 'suspended', 'rejected', 'revoked', 'withdrawn']);
export const partnerGame = z.enum(['fivem', 'minecraft', 'both']);
/** What the bot's check found wrong, or why an application was refused. */
export const partnerProblem = z.enum([
  'not_linked',
  'invalid_link',
  'unknown_invite',
  'bot_missing',
  'too_few_members',
  'not_admin',
  'channel_missing',
  'channel_hidden',
  'channel_link',
  'pending',
  'taken',
  'cooldown',
]);

export const partnerChannelSchema = z.object({
  id: z.string(),
  name: z.string(),
});

export const partnerServerSchema = z.object({
  uuid: z.string(),
  uuidShort: z.string(),
  name: z.string(),
});

export const partnerPaymentSchema = z.object({
  period: z.string(),
  credits: z.number(),
  created: z.coerce.date(),
});

export const partnerApplicationSchema = z.object({
  uuid: z.string(),
  guildId: z.string(),
  guildName: z.string(),
  iconUrl: z.string().nullable(),
  members: z.number(),
  channel: partnerChannelSchema.nullable(),
  game: z.string(),
  description: z.string(),
  servers: z.array(partnerServerSchema),
  status: partnerStatus,
  monthlyCredits: z.number().nullable(),
  nextPayment: z.coerce.date().nullable(),
  problems: z.array(z.string()),
  suspendAfter: z.coerce.date().nullable(),
  payments: z.array(partnerPaymentSchema),
  paidTotal: z.number(),
  reason: z.string().nullable(),
  created: z.coerce.date(),
  approved: z.coerce.date().nullable(),
  decided: z.coerce.date().nullable(),
});

export const partnersSchema = z.object({
  minMembers: z.number(),
  maxCredits: z.number(),
  graceDays: z.number(),
  channelRequired: z.boolean(),
  channelNames: z.array(z.string()),
  channelLinks: z.array(z.string()),
  discord: z
    .object({
      id: z.string(),
      username: z.string().nullable(),
    })
    .nullable(),
  linkUrl: z.string().nullable(),
  botName: z.string(),
  botInviteUrl: z.string().nullable(),
  applications: z.array(partnerApplicationSchema),
  canApply: z.boolean(),
  applyAfter: z.coerce.date().nullable(),
});

/** Why the panel refused an application (a 422's `refusal`). */
export const partnerRefusalSchema = z.object({
  problems: z.array(partnerProblem),
  guildId: z.string().nullable(),
  guildName: z.string().nullable(),
  members: z.number().nullable(),
  minMembers: z.number(),
  channel: partnerChannelSchema.nullable(),
  botInviteUrl: z.string().nullable(),
  applyAfter: z.coerce.date().nullable(),
});

export const partnerApplySchema = z.object({
  server: z.string().trim().min(1).max(200),
  game: partnerGame,
  description: z.string().trim().min(20).max(1000),
  servers: z.array(z.string()).max(10),
});
