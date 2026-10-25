import { z } from 'zod';
import { adminBackupConfigurationSchema } from '@/lib/schemas/admin/backupConfigurations.ts';
import { adminDatabaseAgentHostSchema } from '@/lib/schemas/admin/databaseAgentHosts.ts';
import { adminDatabaseHostSchema } from '@/lib/schemas/admin/databaseHosts.ts';
import { adminDeviceSchema } from '@/lib/schemas/admin/devices.ts';
import { adminLocationSchema } from '@/lib/schemas/admin/locations.ts';
import { adminMountSchema } from '@/lib/schemas/admin/mounts.ts';
import { adminServerBackupSchema, adminServerSchema } from '@/lib/schemas/admin/servers.ts';
import { nullableString } from '@/lib/serialization/transformers.ts';
import { hostnameSchema } from '../generic.ts';

export const adminNodeSchema = z.looseObject({
  uuid: z.string(),
  location: z.lazy(() => adminLocationSchema),
  backupConfiguration: z.lazy(() => adminBackupConfigurationSchema).nullable(),
  name: z.string().min(1).max(255),
  deploymentEnabled: z.boolean(),
  maintenanceEnabled: z.boolean(),
  description: z.preprocess(nullableString, z.string().max(1024).nullable()),
  publicUrl: z.preprocess(
    nullableString,
    z
      .url({ protocol: /^https?$/ })
      .min(3)
      .max(255)
      .nullable(),
  ),
  url: z
    .url({ protocol: /^https?$/ })
    .min(3)
    .max(255),
  sftpHost: z.preprocess(nullableString, hostnameSchema.nullable()),
  sftpPort: z.number().min(0).max(65535),
  memory: z.number().min(0),
  disk: z.number().min(0),
  // plugged.host: shared/src/models/node/placement.rs
  role: z.enum(['game', 'internal', 'limbo']),
  tags: z.array(z.string().min(1).max(64)).max(32),
  srvTarget: z.preprocess(nullableString, hostnameSchema.nullable()),
  // the hardware the public status page names
  cpuModel: z.preprocess(nullableString, z.string().max(64).nullable()),
  memoryType: z.preprocess(nullableString, z.string().max(32).nullable()),
  deploymentPaused: z.boolean(),
  // why the panel paused it (memory, cpu, unreachable, disk); null when an admin did
  deploymentPausedReason: z.enum(['memory', 'cpu', 'unreachable', 'disk']).nullable().optional(),
  created: z.coerce.date(),
});

// plugged.host: node telemetry (shared/src/node_telemetry.rs)
export const adminNodeTelemetrySchema = z.object({
  reachable: z.boolean(),
  refused: z.boolean(),
  responseTimeMs: z.number().nullable(),
  cpuPercent: z.number().nullable(),
  cpuThreads: z.number().nullable(),
  memoryUsed: z.number().nullable(),
  memoryTotal: z.number().nullable(),
  diskUsed: z.number().nullable(),
  diskTotal: z.number().nullable(),
  load1: z.number().nullable(),
  serversRunning: z.number().nullable(),
  pressure: z.number().nullable(),
  consecutiveFailures: z.number(),
  consecutiveOutages: z.number(),
  error: z.string().nullable(),
  lastReachable: z.coerce.date().nullable(),
  sampled: z.coerce.date(),
});

export const adminNodeTelemetryPointSchema = z.object({
  sampled: z.coerce.date(),
  reachable: z.number(),
  responseTimeMs: z.number().nullable(),
  cpuPercent: z.number().nullable(),
  memoryUsed: z.number().nullable(),
  memoryTotal: z.number().nullable(),
  diskUsed: z.number().nullable(),
  diskTotal: z.number().nullable(),
  load1: z.number().nullable(),
  serversRunning: z.number().nullable(),
});

export const adminNodeTelemetryHistorySchema = z.object({
  latest: adminNodeTelemetrySchema.nullable(),
  bucketMinutes: z.number(),
  history: z.array(adminNodeTelemetryPointSchema),
});

export const adminNodeTokenSchema = z.object({
  tokenId: z.string(),
  token: z.string(),
});

export const adminNodeSetupProbeSchema = z.object({
  version: z.string(),
  container: z.boolean(),
  architecture: z.string(),
  cpuCount: z.number(),
  memoryBytes: z.number(),
  diskBytes: z.number(),
  ips: z.array(z.string()),
  apiPort: z.number(),
  sftpPort: z.number(),
  docker: z.object({
    available: z.boolean(),
    version: z.string().nullable(),
  }),
});

export const adminNodeEnrollmentSchema = z.object({
  code: z.string(),
  expires: z.coerce.date(),
});

export const adminNodeAllocatedCapacitySchema = z.object({
  servers: z.number(),
  cpu: z.number(),
  memory: z.number(),
  memoryOverhead: z.number(),
  disk: z.number(),
});

export const adminNodeCapacitySchema = z.object({
  limits: z.object({
    memory: z.number(),
    disk: z.number(),
  }),
  allocated: adminNodeAllocatedCapacitySchema,
  servers: z.array(
    z.object({
      uuid: z.string(),
      name: z.string(),
      suspended: z.boolean(),
      cpu: z.number(),
      memory: z.number(),
      memoryOverhead: z.number(),
      disk: z.number(),
    }),
  ),
});

export const adminNodeCapacitiesSchema = z.record(z.string(), adminNodeAllocatedCapacitySchema);

export const adminNodeUpdateSchema = z.lazy(() =>
  adminNodeSchema
    .omit({
      uuid: true,
      location: true,
      backupConfiguration: true,
      deploymentPausedReason: true, // plugged.host: the panel's own
      created: true,
    })
    .extend({
      locationUuid: z.uuid(),
      backupConfigurationUuid: z.uuid().nullable(),
    }),
);

export const adminNodeServerBackupSchema = z.lazy(() =>
  adminServerBackupSchema.extend({
    node: adminNodeSchema,
  }),
);

export const adminNodeAllocationSchema = z.looseObject({
  uuid: z.string(),
  server: z.lazy(() => adminServerSchema).nullable(),
  ip: z.string(),
  ipAlias: z.string().nullable(),
  port: z.number(),
  created: z.string(),
});

export const adminNodeAllocationFilterSchema = z.object({
  search: z.string().nullable().optional(),
  ip: z.string().nullable().optional(),
  portFrom: z.number().nullable().optional(),
  portTo: z.number().nullable().optional(),
  assigned: z.boolean().nullable().optional(),
});

export const adminNodeAllocationSelectorSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('uuids'), uuids: z.array(z.string()) }),
  z.object({ type: z.literal('all') }),
  z.object({ type: z.literal('filter'), filter: adminNodeAllocationFilterSchema }),
]);

export const adminNodeAllocationsSchema = z.object({
  ip: z.string().min(1),
  ipAlias: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  ports: z.array(z.string()).min(1),
});

export const adminNodeMountSchema = z.looseObject({
  mount: z.lazy(() => adminMountSchema),
  created: z.coerce.date(),
});

export const adminNodeDeviceSchema = z.looseObject({
  device: z.lazy(() => adminDeviceSchema),
  created: z.coerce.date(),
});

export const adminNodeDatabaseHostSchema = z.looseObject({
  databaseHost: z.lazy(() => adminDatabaseHostSchema),
  created: z.coerce.date(),
});

export const adminNodeDatabaseAgentHostSchema = z.looseObject({
  databaseAgentHost: z.lazy(() => adminDatabaseAgentHostSchema),
  created: z.coerce.date(),
});

export const adminNodeTransferProgressSchema = z.object({
  archiveBytesProcessed: z.number(),
  networkBytesProcessed: z.number(),
  bytesTotal: z.number(),
  filesProcessed: z.number(),
});

export const adminNodeTransfersSchema = z.record(z.string(), adminNodeTransferProgressSchema);

export type AdminNode = z.infer<typeof adminNodeSchema>;
export type AdminNodeAllocatedCapacity = z.infer<typeof adminNodeAllocatedCapacitySchema>;
export type AdminNodeToken = z.infer<typeof adminNodeTokenSchema>;
export type AdminNodeSetupProbe = z.infer<typeof adminNodeSetupProbeSchema>;
export type AdminNodeAllocation = z.infer<typeof adminNodeAllocationSchema>;
export type AdminNodeAllocationFilter = z.infer<typeof adminNodeAllocationFilterSchema>;
export type AdminNodeAllocationSelector = z.infer<typeof adminNodeAllocationSelectorSchema>;
export type AdminNodeMount = z.infer<typeof adminNodeMountSchema>;
export type AdminNodeDevice = z.infer<typeof adminNodeDeviceSchema>;
export type AdminNodeDatabaseHost = z.infer<typeof adminNodeDatabaseHostSchema>;
export type AdminNodeDatabaseAgentHost = z.infer<typeof adminNodeDatabaseAgentHostSchema>;
export type AdminNodeServerBackup = z.infer<typeof adminNodeServerBackupSchema>;

export const adminNodeBandwidthSchema = z.object({
  enabled: z.boolean(),
  ready: z.boolean(),
  reason: z.string().nullable(),
});
