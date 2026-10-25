// plugged.host: a node's failovers (shared/src/node_failover.rs, docs/features/failover.md)
import { z } from 'zod';

export const adminServerFailoverSchema = z.object({
  uuid: z.string(),
  serverUuid: z.string(),
  serverName: z.string(),
  destinationNodeUuid: z.string().nullable(),
  destinationNodeName: z.string().nullable(),
  backupCreated: z.coerce.date().nullable(),
  // planned: a server of a notify row, where a failover would put it
  status: z.enum(['queued', 'rehoming', 'restoring', 'done', 'failed', 'cancelled', 'planned']),
  error: z.string().nullable(),
  finished: z.coerce.date().nullable(),
  sourceCleaned: z.coerce.date().nullable(),
});

export const adminNodeFailoverSchema = z.object({
  uuid: z.string(),
  // notify: a down node reported in the notify mode; nothing moved
  mode: z.enum(['auto', 'manual', 'drill', 'notify']),
  status: z.enum(['running', 'done', 'cancelled', 'notified']),
  startedBy: z.string().nullable(),
  created: z.coerce.date(),
  finished: z.coerce.date().nullable(),
  servers: z.array(adminServerFailoverSchema),
});

export const adminNodeFailoversSchema = z.object({
  mode: z.enum(['off', 'manual', 'notify', 'auto']),
  failovers: z.array(adminNodeFailoverSchema),
});

export const adminStartNodeFailoverSchema = z.object({
  drillServerUuid: z.string().nullable(),
});

// a dry run: where each server would go and from which backup, or why it would fail
export const adminFailoverPlanSchema = z.object({
  serverUuid: z.string(),
  serverName: z.string(),
  destinationNodeUuid: z.string().nullable(),
  destinationNodeName: z.string().nullable(),
  backupUuid: z.string().nullable(),
  backupCreated: z.coerce.date().nullable(),
  error: z.string().nullable(),
});

export const adminFailoverPlansSchema = z.object({
  plan: z.array(adminFailoverPlanSchema),
});
