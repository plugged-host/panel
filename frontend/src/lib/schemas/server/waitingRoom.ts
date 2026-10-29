// plugged.host: a server's start waiting room (shared/src/waiting_room.rs)
import { z } from 'zod';

export const serverWaitingRoomSchema = z.object({
  status: z.enum(['waiting', 'released', 'none']),
  elapsed: z.number(),
  minSeconds: z.number(),
  maxSeconds: z.number(),
  running: z.boolean(),
  ready: z.boolean(),
  // plugged.host: the day pass sold in the room, and the second membership that skips it
  dayPass: z
    .object({
      tokens: z.number(),
      hours: z.number(),
      balance: z.number(),
      until: z.coerce.date().nullable(),
    })
    .nullable()
    .default(null),
  premium: z.string().nullable().default(null),
});
