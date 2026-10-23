import { z } from 'zod';
import { nullableString } from '@/lib/serialization/transformers.ts';

export const serverSettingsReinstallSchema = z.object({
  truncateDirectory: z.boolean(),
  startOnCompletion: z.boolean(),
});

export const serverSettingsAutokillSchema = z.object({
  enabled: z.boolean(),
  seconds: z.number().min(1).max(3600),
});

export const serverSettingsAutostartSchema = z.object({
  behavior: z.enum(['always', 'unless_stopped', 'never']),
});

export const serverSettingsRenameSchema = z.object({
  name: z.string().min(1).max(255),
  description: z.preprocess(nullableString, z.string().max(1024).nullable()),
});

export const serverSettingsTimezoneSchema = z.object({
  timezone: z.preprocess((value) => (value === '' ? null : value), z.string().min(3).max(255).nullable()),
});

export const serverTrashSettingsSchema = z.object({
  retention: z.number().int().min(0).max(31536000),
  usageThreshold: z.number().int().min(0).max(100),
});

export const serverSettingsTrashSchema = z.object({
  custom: serverTrashSettingsSchema.nullable(),
});

// plugged.host: resize and software (docs/features/server-settings.md)
const serverResizeResourcesSchema = z.object({
  cpu: z.number(),
  memory: z.number(),
  disk: z.number(),
});

export const serverSettingsResizeOptionsSchema = z.object({
  enabled: z.boolean(),
  current: serverResizeResourcesSchema,
  max: z.object({
    cpu: z.number().nullable(),
    memory: z.number().nullable(),
    disk: z.number().nullable(),
  }),
});

export const serverSettingsResizeSchema = z.object({
  cpu: z.number().int().min(0),
  memory: z.number().int().min(0),
  disk: z.number().int().min(0),
});

export const serverSettingsSoftwareOptionsSchema = z.object({
  variants: z.array(z.object({ value: z.string(), label: z.string() })),
  current: z.string().nullable(),
});

export const serverSettingsSoftwareSchema = z.object({
  variant: z.string().min(1).max(64),
});
