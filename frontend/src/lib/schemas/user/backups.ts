// plugged.host: the account's Backups page (shared/src/deleted_servers.rs, docs/features/backups-page.md)
import { z } from 'zod';

export const userAccountBackupSchema = z.object({
  serverUuid: z.string(),
  serverUuidShort: z.string(),
  serverName: z.string(),
  eggName: z.string().nullable(),
  nodeName: z.string(),
  deleted: z.coerce.date().nullable(),
  keptUntil: z.coerce.date().nullable(),
  backup: z
    .object({
      uuid: z.string(),
      name: z.string(),
      bytes: z.number(),
      created: z.coerce.date(),
    })
    .nullable(),
});
