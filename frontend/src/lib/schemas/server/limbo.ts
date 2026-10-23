// plugged.host: idle servers stopped and parked (shared/src/limbo.rs, docs/features/limbo.md)
import { z } from 'zod';
import { serverCreateLocationSchema } from '@/lib/schemas/server/create.ts';

export const serverLimboStateSchema = z.enum(['stopped', 'parking', 'parked', 'restoring', 'unpacking']);

export const serverLimboSchema = z.object({
  state: serverLimboStateSchema.nullable(),
  lastActivity: z.string().nullable(),
  parked: z.string().nullable(),
  cost: z.number(),
  tokens: z.number().nullable(),
  homeAvailable: z.boolean(),
  homeLocationUuid: z.string(),
  locations: z.array(serverCreateLocationSchema),
  error: z.string().nullable(),
});
