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
  // the account's other noprofit.space services' part of the pool (already in limit and used)
  elsewhere: resourcesSchema,
  servers: z.number(),
  maxServers: z.number(),
});

export const serverCreateGameSchema = z.object({
  eggUuid: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  icon: z.string().nullable(),
  eula: z.boolean(),
  setup: z.boolean(),
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
  full: z.boolean(),
  latency: z.number().nullable(),
  // plugged.host: the nodes' addresses the browser times for the visitor's own ping (lib/locationPing.ts)
  probes: z.array(z.string()).default([]),
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

// A plan, or the player's own CPU, memory and disk (a plan only fills them in on the page).
export const serverCreateSchema = z.object({
  name: z.string().trim().min(1).max(191),
  eggUuid: z.string(),
  locationUuid: z.string(),
  planUuid: z.string().nullable(),
  cpu: z.number().int().min(1),
  memory: z.number().int().min(1),
  disk: z.number().int().min(1),
  variant: z.string().nullable(),
  eula: z.boolean(),
});

export type ServerCreateSize = { cpu: number; memory: number; disk: number };
