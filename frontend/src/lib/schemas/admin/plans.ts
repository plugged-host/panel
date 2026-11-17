// plugged.host: Admin › Plans, the resource presets players create servers with
import { z } from 'zod';
import { nullableString } from '@/lib/serialization/transformers.ts';

export const adminPlanSchema = z.looseObject({
  uuid: z.string(),
  name: z.string().min(1).max(255),
  description: z.preprocess(nullableString, z.string().max(1024).nullable()),
  cpu: z.number().int().min(1),
  memory: z.number().int().min(1),
  disk: z.number().int().min(1),
  sort: z.number().int(),
  public: z.boolean(),
  // a paid plan with CPU threads and memory of its own (docs/features/dedicated.md); price in cents a month
  dedicated: z.boolean().default(false),
  price: z.number().int().min(0).max(100_000_000).nullable().default(null),
  locationUuids: z.array(z.string()).max(100).default([]),
  nodeUuids: z.array(z.string()).max(100).default([]),
  created: z.string(),
});

export const adminPlanUpdateSchema = adminPlanSchema.omit({
  uuid: true,
  created: true,
});
