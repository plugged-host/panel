// plugged.host: Admin › Abuse (shared/src/abuse.rs, docs/features/abuse.md)
import { z } from 'zod';

export const abuseFindingStatus = z.enum(['open', 'actioned', 'dismissed', 'resolved']);
export const abuseSeverity = z.enum(['critical', 'review']);

export const adminAbuseFindingSchema = z.object({
  uuid: z.string(),
  server: z
    .object({
      uuid: z.string(),
      name: z.string(),
      ownerUsername: z.string().nullable(),
      eggName: z.string().nullable(),
    })
    .nullable(),
  node: z
    .object({
      uuid: z.string(),
      name: z.string(),
    })
    .nullable(),
  rule: z.string(),
  category: z.string(),
  categoryLabel: z.string(),
  severity: abuseSeverity,
  source: z.string(),
  status: abuseFindingStatus,
  summary: z.string(),
  evidence: z.unknown(),
  hits: z.number(),
  firstSeen: z.coerce.date(),
  lastSeen: z.coerce.date(),
  action: z.string().nullable(),
  actioned: z.coerce.date().nullable(),
  resolvedBy: z.string().nullable(),
  resolved: z.coerce.date().nullable(),
});

export const adminAbuseScanResultSchema = z.object({
  finished: z.boolean(),
  reachable: z.boolean(),
  findings: z.array(
    z.object({
      rule: z.string(),
      category: z.string(),
      severity: abuseSeverity,
      summary: z.string(),
    }),
  ),
});

export type AdminAbuseFinding = z.infer<typeof adminAbuseFindingSchema>;
export type AdminAbuseScanResult = z.infer<typeof adminAbuseScanResultSchema>;
