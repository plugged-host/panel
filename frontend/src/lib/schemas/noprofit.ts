import { z } from 'zod';

// plugged.host: noprofit.space accounts (backend shared/src/noprofit.rs).

export const noprofitPlanSchema = z.object({
  cpu: z.number(),
  memory: z.number(),
  disk: z.number(),
});

export const noprofitDiscordSchema = z.object({
  id: z.string(),
  username: z.string().nullable(),
  avatar: z.string().nullable(),
});

export const noprofitAccountSchema = z.object({
  accountId: z.number(),
  email: z.string(),
  username: z.string(),
  isAdmin: z.boolean(),
  credits: z.number(),
  plan: noprofitPlanSchema.nullable().catch(null),
  discord: noprofitDiscordSchema.nullable().catch(null),
  synced: z.string(),
});

export const noprofitAccountResponseSchema = z.object({
  accountsUrl: z.string().nullable(),
  account: noprofitAccountSchema.nullable(),
});
