// plugged.host: a server's place in the start queue (shared/src/start_queue.rs)
import { z } from 'zod';

export const serverStartQueueSchema = z.object({
  status: z.enum(['waiting', 'released', 'left', 'none']),
  position: z.number(),
  waited: z.number(),
  remaining: z.number(),
  priority: z.number(),
  membershipOffer: z.boolean(),
  moveOffer: z.boolean(),
});
