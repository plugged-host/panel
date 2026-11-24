// plugged.host: a paid server's month, paid with noprofit.space Funds (shared/src/dedicated.rs, docs/features/dedicated.md)
import { z } from 'zod';

export const serverSubscriptionSchema = z.object({
  // euro cents a month
  price: z.number(),
  paidUntil: z.coerce.date(),
  renews: z.boolean(),
  unpaidSince: z.coerce.date().nullable(),
  deleteAt: z.coerce.date().nullable(),
});

// null: nobody pays for the server here (a free one, or one an admin made)
export const serverBillingSchema = z.object({
  subscription: serverSubscriptionSchema.nullable(),
});

export type ServerSubscription = z.infer<typeof serverSubscriptionSchema>;

// one of the account's paid servers, for the Paid page
export const paidServerSchema = serverSubscriptionSchema.extend({
  uuid: z.string(),
  uuidShort: z.string(),
  name: z.string(),
  plan: z.string().nullable(),
});

export type PaidServer = z.infer<typeof paidServerSchema>;
