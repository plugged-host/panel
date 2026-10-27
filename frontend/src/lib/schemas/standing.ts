// plugged.host: an account's standing, from its suspensions (shared/src/standing.rs, docs/features/standing.md)
import { z } from 'zod';

export const accountStandingSchema = z.object({
  level: z.string(),
  suspensions: z.number(),
  canCreate: z.boolean(),
  createBlockedUntil: z.coerce.date().nullable(),
  // the standing page on noprofit.space; null without it
  url: z.string().nullable(),
});

export const accountStandingResponseSchema = z.object({
  standing: accountStandingSchema.nullable().catch(null),
});
