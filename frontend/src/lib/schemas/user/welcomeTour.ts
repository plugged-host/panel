// plugged.host: the welcome tour's state (shared/src/welcome_tour.rs, docs/features/welcome-tour.md)
import { z } from 'zod';

export const welcomeTourSchema = z.object({
  panelActive: z.boolean(),
  serverActive: z.boolean(),
  step: z.number(),
  serverStep: z.number(),
});

export const welcomeTourAction = z.enum([
  'advance',
  'advance_server',
  'complete',
  'skip',
  'complete_server',
  'restart',
]);

export const welcomeTourUpdateSchema = z.object({
  action: welcomeTourAction,
  step: z.number().int().min(1).max(50).optional(),
});
