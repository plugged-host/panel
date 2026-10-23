// plugged.host: a node's failovers (shared/src/node_failover.rs, docs/features/failover.md)
import { z } from 'zod';

export const adminServerFailoverSchema = z.object({
  uuid: z.string(),
  serverUuid: z.string(),
  serverName: z.string(),
  destinationNodeUuid: z.string().nullable(),
  destinationNodeName: z.string().nullable(),
  backupCreated: z.coerce.date().nullable(),
  status: z.enum(['queued', 'rehoming', 'restoring', 'done', 'failed', 'cancelled']),
  error: z.string().nullable(),
  finished: z.coerce.date().nullable(),
  sourceCleaned: z.coerce.date().nullable(),
});

export const adminNodeFailoverSchema = z.object({
  uuid: z.string(),
  mode: z.enum(['auto', 'manual', 'drill']),
  status: z.enum(['running', 'done', 'cancelled']),
  startedBy: z.string().nullable(),
  created: z.coerce.date(),
  finished: z.coerce.date().nullable(),
  servers: z.array(adminServerFailoverSchema),
});

export const adminNodeFailoversSchema = z.object({
  mode: z.enum(['off', 'manual', 'auto']),
  failovers: z.array(adminNodeFailoverSchema),
});

export const adminStartNodeFailoverSchema = z.object({
  drillServerUuid: z.string().nullable(),
});
