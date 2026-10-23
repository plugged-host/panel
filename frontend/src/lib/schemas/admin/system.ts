import { z } from 'zod';
import { adminDatabaseAgentHostSchema } from './databaseAgentHosts.ts';
import { adminNodeSchema } from './nodes.ts';

export const adminExtensionUpdateCheckResultNoUpdateSchema = z.object({
  type: z.literal('no_update'),
});

export const adminExtensionUpdateCheckResultUpdateAvailableSchema = z.object({
  type: z.literal('update_available'),
  version: z.string(),
  latestVersion: z.string(),
  changes: z.string().array(),
});

export const adminExtensionUpdateCheckResultErrorSchema = z.object({
  type: z.literal('error'),
  error: z.string(),
});

export const adminExtensionUpdateCheckResultSchema = z.discriminatedUnion('type', [
  adminExtensionUpdateCheckResultNoUpdateSchema,
  adminExtensionUpdateCheckResultUpdateAvailableSchema,
  adminExtensionUpdateCheckResultErrorSchema,
]);

export const adminUpdateHistoryEntrySchema = z.object({
  version: z.string(),
  timestamp: z.string(),
});

export const adminUpdateHistorySchema = z.object({
  panel: adminUpdateHistoryEntrySchema.array(),
  extensions: z.record(z.string(), adminUpdateHistoryEntrySchema.array()),
});

export const adminUpdateInformationSchema = z.object({
  panelVersion: z.string(),
  latestPanelVersion: z.string(),
  latestWingsVersion: z.string(),
  latestDbAgentVersion: z.string(),
  extensions: z.record(z.string(), adminExtensionUpdateCheckResultSchema),
});

export const adminUpgradeStatusSchema = z.object({
  currentVersion: z.string(),
  latestVersion: z.string().nullable(),
  updateAvailable: z.boolean(),
  upgradable: z.boolean(),
  source: z.enum(['daemon', 'calagopus']),
});

export const adminNodeUpdateInformationSchema = adminUpgradeStatusSchema.extend({
  node: z.lazy(() => adminNodeSchema),
});

export const adminDatabaseAgentHostUpdateInformationSchema = adminUpgradeStatusSchema.extend({
  databaseAgentHost: z.lazy(() => adminDatabaseAgentHostSchema),
});

export const adminUpgradeSourceSchema = z.object({
  provider: z.string(),
  repository: z.string(),
  apiUrl: z.string(),
});

export const adminUpgradeSchema = z.object({
  version: z.string().trim().min(1).max(64),
});

export const adminUpgradeResultSchema = z.object({
  applied: z.boolean(),
  version: z.string().nullable(),
});

export const adminNodeDesyncSchema = z.object({
  localTime: z.string(),
  panelLocalTime: z.string(),
  node: z.lazy(() => adminNodeSchema),
});

export const adminSystemStatisticsSchema = z.object({
  cpu: z.object({
    used: z.number(),
    threads: z.number(),
    model: z.string(),
  }),
  network: z.object({
    received: z.number(),
    receivingRate: z.number(),
    sent: z.number(),
    sendingRate: z.number(),
  }),
  memory: z.object({
    used: z.number(),
    usedProcess: z.number(),
    total: z.number(),
  }),
  disk: z.object({
    used: z.number(),
    total: z.number(),
    read: z.number(),
    readingRate: z.number(),
    written: z.number(),
    writingRate: z.number(),
  }),
});
