// plugged.host: what a user buys with tokens for one server (docs/features/server-upgrades.md)
import { z } from 'zod';

export const serverUpgradeKindSchema = z.enum([
  'keep_awake',
  'boost',
  'extra_allocation',
  'extra_database',
  'extra_backup',
  'card_color',
]);

export const serverUpgradeOfferSchema = z.object({
  kind: serverUpgradeKindSchema,
  tokens: z.number(),
  // what one purchase or renewal runs; null for good (the card colour)
  hours: z.number().nullable(),
  // a boost's memory (MiB) and CPU (%)
  memory: z.number(),
  cpu: z.number(),
  max: z.number(),
  // why a new one cannot be bought now; renewing one that runs still works
  unavailable: z.enum(['max', 'node_busy', 'node_unknown', 'unlimited', 'parked']).nullable(),
});

export const serverUpgradePurchaseSchema = z.object({
  uuid: z.string(),
  kind: serverUpgradeKindSchema,
  until: z.coerce.date().nullable(),
  created: z.coerce.date(),
});

export const serverUpgradesSchema = z.object({
  offers: z.array(serverUpgradeOfferSchema),
  purchases: z.array(serverUpgradePurchaseSchema),
  cardColor: z.string().nullable(),
  palette: z.array(z.string()),
  balance: z.number(),
});
