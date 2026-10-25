import { z } from 'zod';
import { nullableString } from '@/lib/serialization/transformers.ts';

export const serverAllocationSchema = z.looseObject({
  uuid: z.string(),
  ip: z.string(),
  ipAlias: z.string().nullable(),
  port: z.number(),
  notes: z.string().nullable(),
  isPrimary: z.boolean(),
  locked: z.boolean().default(false), // plugged.host: only an admin may remove it
  resolvedIp: z.string().nullable().default(null), // plugged.host: the IP beside its host name
  created: z.coerce.date(),
});

export const serverAllocationsEditSchema = z.object({
  notes: z.preprocess(nullableString, z.string().max(1024).nullable()),
});
