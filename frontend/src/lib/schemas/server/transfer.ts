// plugged.host: a player moving their server to another location (shared/src/server_transfers.rs, docs/features/transfers.md)
import { z } from 'zod';
import { serverCreateLocationSchema } from '@/lib/schemas/server/create.ts';

export const serverTransferPlaceSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  flag: z.string().nullable(),
});

export const serverTransferRequestSchema = z.object({
  uuid: z.string(),
  origin: z.enum(['user', 'admin', 'limbo']),
  status: z.enum(['queued', 'running', 'cancelling', 'completed', 'failed', 'cancelled']),
  locationUuid: z.string().nullable(),
  attempts: z.number(),
  error: z.string().nullable(),
  reason: z.string().nullable(),
  fromLocation: serverTransferPlaceSchema.nullable(),
  toLocation: serverTransferPlaceSchema.nullable(),
  created: z.coerce.date(),
  finished: z.coerce.date().nullable(),
});

export const serverTransferProgressSchema = z.object({
  state: z.string(),
  step: z.string(),
  percent: z.number(),
  bytesDone: z.number(),
  bytesTotal: z.number(),
  rate: z.number().nullable(),
  etaSeconds: z.number().nullable(),
});

export const serverTransferEventSchema = z.object({
  at: z.coerce.date(),
  key: z.string(),
  params: z.record(z.string(), z.string()),
  level: z.string(),
});

export const serverTransferOptionsSchema = z.object({
  enabled: z.boolean(),
  currentLocationUuid: z.string(),
  currentLocation: serverTransferPlaceSchema,
  locations: z.array(serverCreateLocationSchema),
  cooldownUntil: z.coerce.date().nullable(),
  blocked: z.string().nullable(),
  maxAttempts: z.number(),
  dataBytes: z.number().nullable(),
  backups: z.object({ count: z.number(), bytes: z.number() }),
  estimateSeconds: z.number(),
  request: serverTransferRequestSchema.nullable(),
  progress: serverTransferProgressSchema.nullable(),
  events: z.array(serverTransferEventSchema),
  history: z.array(serverTransferRequestSchema),
});

export const serverTransferStatusSchema = z.object({
  request: serverTransferRequestSchema,
  progress: serverTransferProgressSchema.nullable(),
});

export const serverTransferSchema = z.object({
  locationUuid: z.uuid(),
});

export const activeTransferStatuses = ['queued', 'running', 'cancelling'] as const;
