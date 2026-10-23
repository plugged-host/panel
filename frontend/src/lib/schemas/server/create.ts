// plugged.host: what the create page offers (shared/src/server_creation.rs, docs/features/create-server.md)
import { z } from 'zod';

const resourcesSchema = z.object({
  cpu: z.number(),
  memory: z.number(),
  disk: z.number(),
});

export const serverCreateAllowanceSchema = z.object({
  source: z.enum(['plan', 'settings']),
  hasPlan: z.boolean(),
  limit: resourcesSchema,
  used: resourcesSchema,
  servers: z.number(),
  maxServers: z.number(),
});

export const serverCreateGameSchema = z.object({
  eggUuid: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  eula: z.boolean(),
  variants: z.array(z.object({ value: z.string(), label: z.string() })),
});

export const serverCreateLocationSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  flag: z.string().nullable(),
  tier: z.string().nullable(),
  unlocked: z.boolean(),
  available: z.boolean(),
  latency: z.number().nullable(),
});

export const serverCreatePlanSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  cpu: z.number(),
  memory: z.number(),
  disk: z.number(),
  available: z.boolean(),
});

export const serverCreateOptionsSchema = z.object({
  enabled: z.boolean(),
  allowance: serverCreateAllowanceSchema,
  games: z.array(serverCreateGameSchema),
  locations: z.array(serverCreateLocationSchema),
  plans: z.array(serverCreatePlanSchema),
});

export const serverCreateSchema = z.object({
  name: z.string().trim().min(3).max(255),
  eggUuid: z.string(),
  locationUuid: z.string(),
  planUuid: z.string(),
  variant: z.string().nullable(),
  eula: z.boolean(),
});
