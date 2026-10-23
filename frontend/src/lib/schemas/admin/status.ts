// plugged.host: Admin › Status (shared/src/status.rs, docs/features/status.md)
import { z } from 'zod';

export const statusComponentStatus = z.enum([
  'operational',
  'degraded',
  'maintenance',
  'partial_outage',
  'major_outage',
  'offline',
  'unknown',
]);
export const statusIncidentImpact = z.enum(['minor', 'major', 'critical', 'maintenance']);
export const statusIncidentStatus = z.enum(['investigating', 'identified', 'monitoring', 'resolved']);

export const adminStatusComponentSchema = z.object({
  uuid: z.string(),
  kind: z.enum(['site', 'panel', 'node']),
  nodeUuid: z.string().nullable(),
  label: z.string(),
  published: z.boolean(),
  sort: z.number(),
  status: statusComponentStatus,
  lastReported: z.coerce.date().nullable(),
  checked: z.coerce.date().nullable(),
});

export const adminStatusComponentUpdateSchema = z.object({
  published: z.boolean().optional(),
  sort: z.number().int().min(0).max(100000).optional(),
  label: z.string().min(1).max(255).optional(),
});

export const adminStatusIncidentUpdateSchema = z.object({
  uuid: z.string(),
  status: statusIncidentStatus,
  message: z.string(),
  createdBy: z.string().nullable(),
  published: z.coerce.date(),
});

export const adminStatusIncidentSchema = z.object({
  uuid: z.string(),
  title: z.string(),
  impact: statusIncidentImpact,
  status: statusIncidentStatus,
  components: z.array(z.string()),
  createdBy: z.string().nullable(),
  started: z.coerce.date(),
  resolved: z.coerce.date().nullable(),
  updates: z.array(adminStatusIncidentUpdateSchema),
});

export const adminStatusIncidentCreateSchema = z.object({
  title: z.string().trim().min(1).max(255),
  impact: statusIncidentImpact,
  status: statusIncidentStatus,
  components: z.array(z.string()).max(100),
  message: z.string().trim().min(1).max(5000),
});

export const adminStatusIncidentPostUpdateSchema = z.object({
  status: statusIncidentStatus,
  message: z.string().trim().min(1).max(5000),
});

export const adminStatusOverrideSchema = z.object({
  uuid: z.string(),
  componentUuid: z.string().nullable(),
  status: statusComponentStatus,
  note: z.string().nullable(),
  starts: z.coerce.date().nullable(),
  ends: z.coerce.date().nullable(),
  createdBy: z.string().nullable(),
  created: z.coerce.date(),
  active: z.boolean(),
});

export const adminStatusOverrideCreateSchema = z.object({
  componentUuid: z.string().nullable(),
  status: statusComponentStatus,
  note: z.string().trim().max(1000).nullable(),
  starts: z.coerce.date().nullable(),
  ends: z.coerce.date().nullable(),
});

export type AdminStatusComponent = z.infer<typeof adminStatusComponentSchema>;
export type AdminStatusIncident = z.infer<typeof adminStatusIncidentSchema>;
export type AdminStatusOverride = z.infer<typeof adminStatusOverrideSchema>;
