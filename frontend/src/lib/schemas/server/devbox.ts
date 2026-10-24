// plugged.host: the server's VS Code page (shared/src/devboxes/view.rs, docs/features/devboxes.md)
import { z } from 'zod';

export const serverDevboxStateSchema = z.enum([
  'hidden',
  'locked',
  'none',
  'stopped',
  'starting',
  'stopping',
  'failed',
  'mount_missing',
  'ready',
]);

export const serverDevboxSharedRightsSchema = z.object({
  status: z.boolean(),
  logs: z.boolean(),
  power: z.boolean(),
  console: z.boolean(),
});

export const serverDevboxConnectionSchema = z.object({
  host: z.string(),
  port: z.number(),
  username: z.string(),
  code: z.string().nullable(),
  hostKey: z.string().nullable(),
  sshCommand: z.string(),
  vscodeUrl: z.string(),
  connections: z.number(),
  web: z.boolean(),
});

export const serverDevboxSchema = z.object({
  state: serverDevboxStateSchema,
  kind: z.enum(['personal', 'shared']).nullable(),
  isOwner: z.boolean(),
  shared: z.boolean(),
  ownerAllowed: z.boolean(),
  sharedRights: serverDevboxSharedRightsSchema.nullable(),
  canStop: z.boolean(),
  polling: z.boolean(),
  idleMinutes: z.number(),
  connection: serverDevboxConnectionSchema.nullable(),
});
