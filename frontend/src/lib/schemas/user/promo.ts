// plugged.host: the membership popup (shared/src/promos.rs)
import { z } from 'zod';

export const userPromoPerkSchema = z.discriminatedUnion('id', [
  z.object({ id: z.literal('locations') }),
  z.object({ id: z.literal('queue'), first: z.boolean() }),
  z.object({ id: z.literal('burst_cores'), cores: z.number() }),
  z.object({ id: z.literal('no_ads') }),
  z.object({ id: z.literal('fewer_ads') }),
  z.object({ id: z.literal('panel_access') }),
  z.object({ id: z.literal('neo'), percent: z.number() }),
  z.object({ id: z.literal('faster_backups') }),
]);

export const userPromoSchema = z.object({
  membership: z.enum(['plus', 'plusplus']),
  name: z.string(),
  cost: z.number(),
  fullCost: z.number(),
  days: z.number(),
  tokens: z.number(),
  // a perk this page does not know yet is left out rather than failing the whole popup
  perks: z.array(z.unknown()).transform((perks) =>
    perks.flatMap((perk) => {
      const parsed = userPromoPerkSchema.safeParse(perk);
      return parsed.success ? [parsed.data] : [];
    }),
  ),
});
