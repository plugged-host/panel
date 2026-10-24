// plugged.host: the account's Discord rewards page (shared/src/discord/rewards.rs, docs/features/discord.md)
import { z } from 'zod';
import { economySourceSchema, economyTransactionSchema } from '@/lib/schemas/user/economy.ts';

export const discordRewardsSchema = z.object({
  source: economySourceSchema,
  pays: z.boolean(),
  account: z
    .object({
      id: z.string(),
      username: z.string().nullable(),
      avatarUrl: z.string().nullable(),
    })
    .nullable(),
  inGuild: z.boolean().nullable(),
  checked: z.coerce.date().nullable(),
  linkUrl: z.string().nullable(),
  noprofitUrl: z.string().nullable(),
  inviteUrl: z.string().nullable(),
  rules: z.object({
    creditsPerInvite: z.number(),
    minAccountAgeDays: z.number(),
    inviteMaturityHours: z.number(),
    inviteClawback: z.boolean(),
    boostOneCredits: z.number(),
    boostTwoCredits: z.number(),
    tier: z.object({
      cpu: z.number(),
      memory: z.number(),
      disk: z.number(),
    }),
  }),
  invites: z.object({
    paid: z.number(),
    waiting: z.number(),
    left: z.number(),
    tooNew: z.number(),
    credits: z.number(),
    nextPayout: z.coerce.date().nullable(),
    owed: z.number(),
  }),
  boosts: z
    .object({
      level: z.number(),
      since: z.coerce.date().nullable(),
      paidLevels: z.array(z.number()),
    })
    .nullable(),
  earned: z.number(),
  history: z.array(economyTransactionSchema),
});
