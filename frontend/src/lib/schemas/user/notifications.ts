// plugged.host: the notification inbox (elements/notifications, docs/features/notifications.md)
import { z } from 'zod';

export const userNotificationSchema = z.looseObject({
  uuid: z.string(),
  // which text it is (`transferCompleted`, `admin.transferFailed`, ...), drawn from `data`
  kind: z.string(),
  data: z.json(),
  // the server it is about, while that server exists
  server: z
    .object({
      uuid: z.string(),
      uuidShort: z.string(),
    })
    .nullable(),
  read: z.coerce.date().nullable(),
  created: z.coerce.date(),
});
