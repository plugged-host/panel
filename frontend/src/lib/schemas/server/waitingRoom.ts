// plugged.host: a server's start waiting room (shared/src/waiting_room.rs)
import { z } from 'zod';

export const serverWaitingRoomSchema = z.object({
  status: z.enum(['waiting', 'released', 'none']),
  elapsed: z.number(),
  minSeconds: z.number(),
  maxSeconds: z.number(),
  running: z.boolean(),
  ready: z.boolean(),
});
