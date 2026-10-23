// plugged.host: a player moving their server to another location (shared/src/server_transfers.rs, docs/features/transfers.md)
import { z } from 'zod';
import { serverCreateLocationSchema } from '@/lib/schemas/server/create.ts';

export const serverTransferRequestSchema = z.object({
  uuid: z.string(),
  origin: z.enum(['user', 'admin']),
  status: z.enum(['queued', 'running', 'cancelling', 'completed', 'failed', 'cancelled']),
  locationUuid: z.string().nullable(),
  attempts: z.number(),
  error: z.string().nullable(),
  created: z.coerce.date(),
  finished: z.coerce.date().nullable(),
});

export const serverTransferOptionsSchema = z.object({
  enabled: z.boolean(),
  currentLocationUuid: z.string(),
  locations: z.array(serverCreateLocationSchema),
  cooldownUntil: z.coerce.date().nullable(),
  request: serverTransferRequestSchema.nullable(),
});

export const serverTransferSchema = z.object({
  locationUuid: z.uuid(),
});

export const activeTransferStatuses = ['queued', 'running', 'cancelling'] as const;
