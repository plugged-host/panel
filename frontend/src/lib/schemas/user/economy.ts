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
      // what it costs without the sale; the price when none runs
      fullPrice: z.number(),
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
  // the sale the prices are on, while one runs (shared/src/economy.rs ApiDiscount)
  discount: z
    .object({
      percent: z.number(),
      name: z.string().nullable(),
      ends: z.coerce.date().nullable(),
    })
    .nullable(),
});

export const economyPaySchema = z.enum(['tokens', 'credits']);

// plugged.host: one of the two memberships, as routes name them (shared/src/economy.rs MembershipKind)
export const membershipKindSchema = z.enum(['plus', 'plusplus']);

// the Resource Store's sale while it covers the memberships (shared/src/economy.rs ApiDiscount)
const membershipDiscountSchema = z.object({
  percent: z.number(),
  name: z.string().nullable(),
  ends: z.coerce.date().nullable(),
});

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
  plusUntil: z.coerce.date().nullable(),
  plusplusUntil: z.coerce.date().nullable(),
  priority: z.number(),
  plusCost: z.number(),
  // plugged.host: without the membership sale; plusCost when none runs
  plusFullCost: z.number(),
  plusDays: z.number(),
  panelAccess: z.boolean(),
  waitingRoom: z.boolean(),
  idleStop: z.boolean(),
  idleDelete: z.boolean(),
  plusplusOffers: z.array(
    z.object({
      pay: economyPaySchema,
      cost: z.number(),
      fullCost: z.number(), // plugged.host: without the membership sale
      days: z.number(),
    }),
  ),
  // plugged.host: the sale while it covers the memberships, automatic renewal, gifts
  // (docs/features/economy.md#memberships)
  discount: membershipDiscountSchema.nullable(),
  autoRenewEnabled: z.boolean(),
  plusAutoRenew: z.boolean(),
  plusplusAutoRenew: z.boolean(),
  giftsEnabled: z.boolean(),
  giftsLeft: z.number(),
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
  // plugged.host: what the second membership includes on another panel, while the bundle runs
  // (shared/src/membership_bundle.rs)
  bundleName: z.string().nullable(),
  // Plus that comes with the other panel's second membership, while it runs, and that membership's name
  bundlePlusUntil: z.coerce.date().nullable(),
  bundleSourceName: z.string(),
});

export const earnProviderSchema = z.enum([
  'cpx_research',
  'linkvertise',
  'vylo',
  'cuty',
  'shrinkearn',
  'shrinkme',
  'adfocus',
]);

export const panelAccessSchema = z.object({
  enabled: z.boolean(),
  required: z.boolean(),
  exempt: z.boolean(),
  tokens: z.number(),
  days: z.number(),
  until: z.coerce.date().nullable(),
});

// plugged.host: the Tokens page's streak, double-token window, first-week tasks and referrals
// (shared/src/{streak,starter_tasks,referrals}.rs, docs/features/economy.md)
export const tokenBonusSchema = z.object({
  percent: z.number(),
  name: z.string().nullable(),
  starts: z.coerce.date().nullable(),
  ends: z.coerce.date().nullable(),
});

export const tokenStreakSchema = z.object({
  day: z.number(),
  reward: z.number(),
  nextReward: z.number(),
  claimable: z.boolean(),
});

export const starterTaskSchema = z.enum(['discord', 'create_server', 'start_server', 'earn']);

export const tokenStarterSchema = z.object({
  reward: z.number(),
  ends: z.coerce.date(),
  tasks: z.array(
    z.object({
      task: starterTaskSchema,
      done: z.boolean(),
      claimed: z.boolean(),
    }),
  ),
});

export const streakClaimSchema = z.object({
  day: z.number(),
  reward: z.number(),
  amount: z.number(),
  streak: tokenStreakSchema,
});

export const userReferralsSchema = z.object({
  code: z.string(),
  url: z.string(),
  reward: z.number(),
  threshold: z.number(),
  monthlyCap: z.number(),
  paidThisMonth: z.number(),
  total: z.number(),
  friends: z.array(
    z.object({
      username: z.string(),
      tokensEarned: z.number(),
      paid: z.coerce.date().nullable(),
    }),
  ),
});

export const userTokensSchema = z.object({
  enabled: z.boolean(),
  tokens: z.number(),
  tokensEarned: z.number(),
  // plugged.host: the part of a token saved toward the next one (fractional link rewards)
  tokenFraction: z.number().default(0),
  claimMinutes: z.number(),
  providers: z.array(
    z.object({
      provider: earnProviderSchema,
      reward: z.number(),
      // plugged.host: what the next link pays today (less with each link done, shared/src/earn.rs)
      nextReward: z.number(),
      dailyCap: z.number(),
      grantedToday: z.number(),
      // plugged.host: a survey wall (CPX Research): pays what each survey shows (reward 0), opens in a
      // new tab, the tokens come with its postback
      survey: z.boolean().default(false),
      fullScript: z.boolean(),
      publisher: z.number().nullable(),
      whitelist: z.array(z.string()),
    }),
  ),
  transactions: z.array(economyTransactionSchema),
  // plugged.host: the double-token window, streak, first-week tasks; referrals has its own route
  bonus: tokenBonusSchema.nullable().default(null),
  streak: tokenStreakSchema.nullable().default(null),
  starter: tokenStarterSchema.nullable().default(null),
  referrals: z.boolean().default(false),
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
  creditsChangeable: z.boolean(),
  extras: z.object({
    cpu: z.number(),
    memory: z.number(),
    disk: z.number(),
  }),
  plusplusPurchases: z.number(),
  creditTransactions: z.array(economyTransactionSchema),
  tokenTransactions: z.array(economyTransactionSchema),
  // plugged.host: who brought the account and how many it brought (docs/features/referrals.md)
  referrals: z
    .object({
      referredBy: z.object({ uuid: z.string(), username: z.string() }).nullable(),
      referred: z.number(),
      paid: z.number(),
    })
    .nullable()
    .default(null),
});

// the sidebar's card (shared/src/economy.rs ApiBalance)
export const userBalanceSchema = z.object({
  credits: z.number().nullable(),
  tokens: z.number().nullable(),
  allowance: serverCreateAllowanceSchema,
  // plugged.host: today's streak tokens wait on the Tokens page (shared/src/streak.rs)
  streakClaimable: z.boolean().default(false),
  // plugged.host: the cheapest membership not had, which the tokens row counts toward
  membershipGoal: z
    .object({
      membership: membershipKindSchema,
      name: z.string(),
      cost: z.number(),
    })
    .nullable()
    .default(null),
});
