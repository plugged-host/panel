import { z } from 'zod';

export const iconSchema = z.object({
  width: z.number(),
  height: z.number(),
  path: z.string(),
});

export const iconsSchema = z.object({
  icons: z.record(z.string(), iconSchema),
});
