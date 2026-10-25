// plugged.host: the Minecraft pages (shared/src/minecraft/, docs/features/minecraft.md)
import { z } from 'zod';

export const minecraftLoaderSchema = z.enum(['paper', 'fabric', 'quilt', 'forge', 'neoforge']);
export const minecraftKindSchema = z.enum(['plugin', 'mod', 'modpack']);
export const minecraftSourceSchema = z.enum(['modrinth', 'curseforge', 'spigotmc']);

export const minecraftSoftwareSchema = z.object({
  loader: minecraftLoaderSchema.nullable(),
  value: z.string().nullable(),
  version: z.string(),
});

export const minecraftContentOptionsSchema = z.object({
  software: minecraftSoftwareSchema,
  gameVersion: z.string().nullable(),
  sources: z.array(minecraftSourceSchema),
  kinds: z.array(minecraftKindSchema),
  directory: z.string().nullable(),
  modpacks: z.boolean(),
  gameVersions: z.array(z.string()),
});

export const minecraftProjectSchema = z.object({
  source: minecraftSourceSchema,
  id: z.string(),
  slug: z.string(),
  title: z.string(),
  description: z.string(),
  author: z.string(),
  downloads: z.number(),
  icon: z.string().nullable(),
  categories: z.array(z.string()),
  url: z.string(),
  kind: minecraftKindSchema,
  installable: z.boolean(),
  clientOnly: z.boolean(),
});

export const minecraftSearchSchema = z.object({
  projects: z.object({
    hits: z.array(minecraftProjectSchema),
    total: z.number(),
    page: z.number(),
    perPage: z.number(),
  }),
  kind: minecraftKindSchema,
  gameVersion: z.string().nullable(),
});

export const minecraftVersionSchema = z.object({
  id: z.string(),
  projectId: z.string(),
  name: z.string(),
  number: z.string(),
  channel: z.string(),
  date: z.string(),
  gameVersions: z.array(z.string()),
  loaders: z.array(z.string()),
  file: z
    .object({
      name: z.string(),
      url: z.string().nullable(),
      size: z.number(),
    })
    .nullable(),
  dependencies: z.array(z.string()),
});

export const minecraftVersionsSchema = z.object({
  versions: z.array(minecraftVersionSchema),
  loose: z.boolean(),
});

export const minecraftInstalledSchema = z.object({
  file: z.string(),
  directory: z.string(),
  version: minecraftVersionSchema,
  loose: z.boolean(),
  dependencies: z.array(z.string()),
});

export const minecraftConfidenceSchema = z.enum(['hash', 'fingerprint', 'name', 'metadata', 'unknown']);

export const minecraftInstalledJarSchema = z.object({
  file: z.string(),
  disabled: z.boolean(),
  size: z.number(),
  modified: z.coerce.date(),
  pending: z.boolean(),
  name: z.string(),
  version: z.string().nullable(),
  authors: z.array(z.string()),
  description: z.string().nullable(),
  kind: minecraftKindSchema.nullable(),
  source: minecraftSourceSchema.nullable(),
  projectId: z.string().nullable(),
  url: z.string().nullable(),
  icon: z.string().nullable(),
  confidence: minecraftConfidenceSchema,
  depends: z.array(z.string()),
  missing: z.array(z.string()),
  dataFolder: z.string().nullable(),
  update: z.object({ id: z.string(), number: z.string() }).nullable(),
});

export const minecraftInstalledListSchema = z.object({
  installed: z.array(minecraftInstalledJarSchema),
  pending: z.number(),
});

export const minecraftInstallSchema = z.object({
  source: minecraftSourceSchema,
  project: z.string(),
  version: z.string().optional(),
});

export const minecraftConfigFieldSchema = z.object({
  path: z.array(z.string()),
  kind: z.enum(['toggle', 'number', 'choice', 'text']),
  value: z.union([z.boolean(), z.number(), z.string(), z.null()]),
  choices: z.array(z.string()),
});

export const minecraftConfigFileSchema = z.object({
  id: z.string(),
  path: z.string(),
  format: z.enum(['properties', 'yaml']),
  fields: z.array(minecraftConfigFieldSchema),
});

export const minecraftConfigSchema = z.object({
  files: z.array(minecraftConfigFileSchema),
});

export const minecraftPlayerSchema = z.object({
  name: z.string(),
  uuid: z.string(),
  whitelisted: z.boolean(),
  operator: z.number().nullable(),
  ban: z
    .object({
      reason: z.string(),
      created: z.string(),
      expires: z.string(),
      source: z.string(),
    })
    .nullable(),
});

export const minecraftRosterSchema = z.object({
  players: z.array(minecraftPlayerSchema),
  enforced: z.boolean(),
  onlineMode: z.boolean(),
  level: z.number(),
  running: z.boolean(),
});

export const minecraftPlayerListSchema = z.enum(['whitelist', 'operators', 'bans']);

export const minecraftOutcomeSchema = z.object({
  live: z.boolean(),
});

export const minecraftWorldSchema = z.object({
  name: z.string(),
  active: z.boolean(),
  exists: z.boolean(),
  folders: z.array(z.string()),
  modified: z.coerce.date().nullable(),
});

export const minecraftGenerationSchema = z.object({
  name: z.string(),
  seed: z.string(),
  levelType: z.string(),
  structures: z.boolean(),
  generator: z.string(),
  datapacks: z.array(z.string()),
});

export const minecraftGameplaySchema = z.object({
  difficulty: z.string(),
  gamemode: z.string(),
  hardcore: z.boolean(),
});

export const minecraftWorldImportSchema = z.object({
  archive: z.string(),
  state: z.enum(['running', 'done', 'failed']),
  error: z.string().nullable(),
  kept: z.string().nullable(),
  started: z.coerce.date(),
});

export const minecraftWorldsSchema = z.object({
  worlds: z.array(minecraftWorldSchema),
  generation: minecraftGenerationSchema,
  gameplay: minecraftGameplaySchema,
  levelTypes: z.array(z.string()),
  running: z.boolean(),
  trash: z.boolean(),
  import: minecraftWorldImportSchema.nullable(),
});

export const minecraftReplacedSchema = z.object({
  kept: z.string().nullable(),
  removed: z.array(z.string()),
});

export const minecraftLogCheckSchema = z.object({
  id: z.string(),
  url: z.string(),
  errors: z.number(),
  problems: z.array(z.object({ message: z.string(), solution: z.string().nullable() })),
  checked: z.coerce.date(),
});

export const minecraftLogCheckResponseSchema = z.object({
  enabled: z.boolean(),
  result: minecraftLogCheckSchema.nullable(),
});

export const minecraftBedrockSchema = z.object({
  available: z.boolean(),
  installed: z.string().nullable(),
  port: z.number().nullable(),
  address: z.string().nullable(),
  running: z.boolean(),
});

export const minecraftBedrockInstalledSchema = z.object({
  updated: z.boolean(),
  floodgate: z.boolean(),
  port: z.number(),
  address: z.string(),
  restarted: z.boolean(),
});

export const minecraftModpackStepSchema = z.enum(['download', 'software', 'files', 'overrides', 'done', 'failed']);

export const minecraftModpackInstallSchema = z.object({
  source: minecraftSourceSchema,
  project: z.string(),
  version: z.string(),
  name: z.string(),
  step: minecraftModpackStepSchema,
  files: z.number(),
  total: z.number(),
  skipped: z.array(z.string()),
  overridesSkipped: z.boolean(),
  minecraft: z.string().nullable(),
  loader: minecraftLoaderSchema.nullable(),
  error: z.string().nullable(),
  started: z.coerce.date(),
});

export const minecraftModpacksSchema = z.object({
  loaders: z.array(minecraftLoaderSchema),
  sources: z.array(minecraftSourceSchema),
  running: z.boolean(),
  install: minecraftModpackInstallSchema.nullable(),
});

export const minecraftSearchPageSchema = minecraftSearchSchema.shape.projects;

export const minecraftOnboardingStepSchema = z.enum([
  'waiting',
  'software',
  'modpack',
  'settings',
  'bedrock',
  'starting',
  'done',
  'failed',
]);

export const minecraftOnboardingProgressSchema = z.object({
  step: minecraftOnboardingStepSchema,
  error: z.string().nullable(),
  skipped: z.array(z.string()),
  started: z.coerce.date(),
});

export const minecraftOnboardingSchema = z.object({
  pending: z.boolean(),
  completed: z.coerce.date().nullable(),
  progress: minecraftOnboardingProgressSchema.nullable(),
  bedrockVariants: z.array(z.string()),
  bedrockLoaders: z.array(minecraftLoaderSchema),
  bedrockNow: z.boolean(),
  modpackLoaders: z.record(z.string(), minecraftLoaderSchema),
});
