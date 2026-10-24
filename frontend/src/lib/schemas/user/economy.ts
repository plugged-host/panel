// plugged.host: credits, tokens, the memberships and the Resource Store (shared/src/economy.rs,
// shared/src/earn.rs, docs/features/economy.md)
import { z } from 'zod';
import { serverCreateAllowanceSchema } from '@/lib/schemas/server/create.ts';

export const economySourceSchema = z.enum(['panel', 'noprofit']);

export const economyTransactionSchema = z.object({
  amount: z.number(),
  balanceAfter: z.number(),
  type: z.string(),
  description: z.string(),
  created: z.coerce.date().nullable(),
});

export const storeResourceSchema = z.enum(['cpu', 'memory', 'disk']);

export const storeCatalogEntrySchema = z.object({
  resource: storeResourceSchema,
  step: z.number(),
  total: z.number(),
  limit: z.number(),
  softLimit: z.number(),
  offers: z.array(
    z.object({
      steps: z.number(),
      amount: z.number(),
      price: z.number(),
    }),
  ),
});

const storeResourcesSchema = z.object({
  cpu: z.number(),
  memory: z.number(),
  disk: z.number(),
});

// what the account has not claimed yet (shared/src/economy.rs ApiPerk)
export const storePerkSchema = z.discriminatedUnion('id', [
  z.object({ id: z.literal('discord_link'), gain: storeResourcesSchema, url: z.string() }),
  z.object({ id: z.literal('discord_join'), gain: storeResourcesSchema, url: z.string() }),
  z.object({ id: z.literal('discord_earn'), perInvite: z.number(), perBoost: z.number(), url: z.string() }),
  z.object({ id: z.literal('store'), price: z.number(), amount: z.number() }),
]);

export const userStoreSchema = z.object({
  source: economySourceSchema,
  available: z.boolean(),
  credits: z.number(),
  catalog: z.array(storeCatalogEntrySchema),
  transactions: z.array(economyTransactionSchema),
  hasPlan: z.boolean(),
  usage: z.array(
    z.object({
      resource: storeResourceSchema,
      used: z.number(),
      limit: z.number(),
    }),
  ),
  rewardsUrl: z.string().nullable(),
  perks: z.array(storePerkSchema),
});

export const economyPaySchema = z.enum(['tokens', 'credits']);

const devboxSizeSchema = z.object({
  cpu: z.number(),
  memory: z.number(),
  disk: z.number(),
});

export const userEconomySchema = z.object({
  source: economySourceSchema,
  creditsEnabled: z.boolean(),
  credits: z.number(),
  tokensEnabled: z.boolean(),
  tokens: z.number(),
  tokensEarned: z.number(),
  membershipEnabled: z.boolean(),
  plusName: z.string(),
  plusplusName: z.string(),
  plusSince: z.coerce.date().nullable(),
  plusplusUntil: z.coerce.date().nullable(),
  priority: z.number(),
  plusCost: z.number(),
  plusplusOffers: z.array(
    z.object({
      pay: economyPaySchema,
      cost: z.number(),
      days: z.number(),
    }),
  ),
  plusBurstCores: z.number(),
  plusplusBurstCores: z.number(),
  burstCores: z.number(),
  neoPlusPercent: z.number().nullable(),
  neoPlusplusPercent: z.number().nullable(),
  devbox: z
    .object({
      membersOnly: z.boolean(),
      standard: devboxSizeSchema,
      premium: devboxSizeSchema,
    })
    .nullable(),
});

export const earnProviderSchema = z.enum(['linkvertise', 'vylo']);

export const userTokensSchema = z.object({
  enabled: z.boolean(),
  tokens: z.number(),
  tokensEarned: z.number(),
  claimMinutes: z.number(),
  providers: z.array(
    z.object({
      provider: earnProviderSchema,
      reward: z.number(),
      dailyCap: z.number(),
      grantedToday: z.number(),
      fullScript: z.boolean(),
      publisher: z.number().nullable(),
      whitelist: z.array(z.string()),
    }),
  ),
  transactions: z.array(economyTransactionSchema),
});

export const earnStartSchema = z.object({
  url: z.string(),
  nonce: z.string(),
});

export const redemptionSchema = z.object({
  status: z.enum(['granted', 'settled', 'no_claim', 'expired', 'no_token', 'replay', 'capped', 'invalid']),
  amount: z.number(),
  provider: earnProviderSchema.nullable(),
});

export const adminUserEconomySchema = userEconomySchema.extend({
  panelCredits: z.number(),
  extras: z.object({
    cpu: z.number(),
    memory: z.number(),
    disk: z.number(),
  }),
  plusplusPurchases: z.number(),
  creditTransactions: z.array(economyTransactionSchema),
  tokenTransactions: z.array(economyTransactionSchema),
});

// the sidebar's card (shared/src/economy.rs ApiBalance)
export const userBalanceSchema = z.object({
  credits: z.number().nullable(),
  tokens: z.number().nullable(),
  allowance: serverCreateAllowanceSchema,
});
