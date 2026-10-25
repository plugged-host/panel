// plugged.host: a server's place in the start queue (shared/src/start_queue.rs)
import { z } from 'zod';

export const serverStartQueueSchema = z.object({
  status: z.enum(['waiting', 'released', 'left', 'none']),
  action: z.enum(['start', 'restart']),
  position: z.number(),
  waited: z.number(),
  remaining: z.number(),
  maxWait: z.number(),
  priority: z.number(),
  memoryPercent: z.number().nullable(),
  cpuPercent: z.number().nullable(),
  line: z.number(),
  reason: z.enum(['memory', 'cpu']).nullable(),
  membershipOffer: z.boolean(),
  plusCost: z.number(),
  passMinutes: z.number(),
  moveNode: z.string().nullable(),
  moveTooBig: z.boolean(),
});
