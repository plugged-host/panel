// plugged.host: the FiveM pages' API shapes (docs/features/fivem.md)
import { z } from 'zod';

export const fivemConnectionSchema = z.object({
  host: z.string(),
  port: z.string(),
  user: z.string(),
  name: z.string(),
  password: z.string(),
  keyValue: z.boolean(),
});

export const fivemSettingsSchema = z.object({
  hostname: z.string(),
  projectName: z.string(),
  projectDescription: z.string(),
  maxClients: z.string(),
  locale: z.string(),
  icon: z.string(),
  gameBuild: z.string(),
  licenseKey: z.string(),
  steamKey: z.string(),
  tebexSecret: z.string(),
  tags: z.array(z.string()),
  database: fivemConnectionSchema,
});

export const fivemResourceSchema = z.object({
  key: z.string().nullable(),
  name: z.string(),
  enabled: z.boolean(),
});

export const fivemPrincipalSchema = z.object({
  key: z.string().nullable(),
  child: z.string(),
  parent: z.string(),
});

export const fivemAceSchema = z.object({
  key: z.string().nullable(),
  principal: z.string(),
  object: z.string(),
  access: z.enum(['allow', 'deny']),
});

export const fivemConfigSchema = z.object({
  path: z.string().nullable(),
  hash: z.string().nullable(),
  endpoints: z.array(z.string()),
  redm: z.boolean(),
  settings: fivemSettingsSchema,
  resources: z.array(fivemResourceSchema),
  principals: z.array(fivemPrincipalSchema),
  aces: z.array(fivemAceSchema),
});

export const fivemPlayerSchema = z.object({
  id: z.number(),
  name: z.string(),
  ping: z.number(),
  identifiers: z.array(z.string()),
});

export const fivemPlayersSchema = z.object({
  online: z.boolean(),
  hostname: z.string().nullable(),
  clients: z.number(),
  slots: z.number().nullable(),
  players: z.array(fivemPlayerSchema),
});

export const fivemEditionSchema = z.enum(['legacy', 'enhanced']);

export const fivemOutcomeSchema = z.enum(['restarting', 'next_start']);

export const fivemBuildSchema = z.object({
  build: z.number(),
  recommended: z.boolean(),
  latest: z.boolean(),
  installed: z.boolean(),
  selected: z.boolean(),
  supportEnds: z.string().nullable(),
});

export const fivemArtifactsSchema = z.object({
  version: z.string(),
  edition: fivemEditionSchema,
  installed: z
    .object({
      edition: fivemEditionSchema,
      build: z.number().nullable(),
      installedAt: z.string().nullable(),
    })
    .nullable(),
  unreachable: z.boolean(),
  onImage: z.boolean(),
  changeable: z.boolean(),
  pending: z.number().nullable(),
  builds: z.array(fivemBuildSchema),
  enhancedBuild: z.number().nullable(),
  enhancedAvailable: z.boolean(),
  redm: z.boolean(),
});

export type FivemVersionChoice =
  | { mode: 'recommended' }
  | { mode: 'latest' }
  | { mode: 'pin' }
  | { mode: 'build'; build: number };

export const fivemRecipeSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  author: z.string().nullable(),
  version: z.string().nullable(),
  logo: z.string().nullable(),
  needsDatabase: z.boolean(),
  tasks: z.number(),
});

export const fivemFrameworkStepSchema = z.enum(['prepare', 'tasks', 'config', 'restart', 'done', 'failed']);

export const fivemFrameworkInstallSchema = z.object({
  recipe: z.string(),
  step: fivemFrameworkStepSchema,
  task: z.number(),
  total: z.number(),
  current: z.string().nullable(),
  deploy: z.string().nullable(),
  previous: z.string().nullable(),
  database: z.string().nullable(),
  needsAdmin: z.boolean(),
  error: z.string().nullable(),
  started: z.string(),
  updated: z.string(),
});

export const fivemFrameworksSchema = z.object({
  game: z.enum(['fivem', 'redm']),
  current: z.string().nullable(),
  unreachable: z.boolean(),
  recipes: z.array(fivemRecipeSchema),
  install: fivemFrameworkInstallSchema.nullable(),
  databases: z.array(z.object({ uuid: z.string(), name: z.string() })),
  canCreateDatabase: z.boolean(),
  license: z.string().nullable(),
});

export type FivemDatabaseChoice = { mode: 'new' } | { mode: 'existing'; uuid: string };
