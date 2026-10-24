// plugged.host: neo's model slots, their status and a test call (backend/src/routes/api/admin/system/neo/)
import { z } from 'zod';

export const adminNeoProviderStatusSchema = z.object({
  slot: z.number(),
  usable: z.boolean(),
  coolingDownUntil: z.number().nullable(),
  tokensToday: z.number(),
  lastError: z
    .object({
      at: z.number(),
      reason: z.string(),
      status: z.number().nullable(),
    })
    .nullable(),
});

export const adminNeoProvidersSchema = z.object({
  providers: z.array(adminNeoProviderStatusSchema),
});

export const adminNeoProviderTestSchema = z.object({
  successful: z.boolean(),
  durationMs: z.number(),
  model: z.string(),
  answer: z.string().nullable(),
  tokens: z.number(),
  status: z.number().nullable(),
  error: z.string().nullable(),
});
