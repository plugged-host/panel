// plugged.host: Admin › Overview › Capacity (shared/src/server_transfers.rs, docs/features/transfers.md)
import { z } from 'zod';
import { serverTransferRequestSchema } from '@/lib/schemas/server/transfer.ts';

export const adminLocationCapacitySchema = z.object({
  uuid: z.string(),
  name: z.string(),
  nodes: z.number(),
  openNodes: z.number(),
  memoryFree: z.number(),
  memoryTotal: z.number(),
  diskFree: z.number(),
  diskTotal: z.number(),
  pressure: z.number().nullable(),
});

export const adminTransferRequestSchema = serverTransferRequestSchema.extend({
  serverUuid: z.string(),
  serverName: z.string(),
  sourceNodeName: z.string().nullable(),
  destinationNodeName: z.string().nullable(),
});

export const adminCapacitySchema = z.object({
  locations: z.array(adminLocationCapacitySchema),
  transfers: z.array(adminTransferRequestSchema),
});

export const adminLightenNodeSchema = z.object({
  pick: z.enum(['stopped', 'busiest']),
  count: z.number().int().min(1).max(100),
});
