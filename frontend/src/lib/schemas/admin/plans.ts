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
  created: z.string(),
});

export const adminPlanUpdateSchema = adminPlanSchema.omit({
  uuid: true,
  created: true,
});
