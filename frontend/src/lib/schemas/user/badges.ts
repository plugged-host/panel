// plugged.host: profile badges (shared/src/badges.rs)
import { z } from 'zod';

export const userBadgesSchema = z.object({
  enabled: z.boolean(),
  tokens: z.number(),
  available: z.array(z.string()),
  owned: z.array(z.string()),
  shown: z.string().nullable(),
  balance: z.number(),
});
