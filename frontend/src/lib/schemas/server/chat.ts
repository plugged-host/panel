import { z } from 'zod';
import { userSchema } from '@/lib/schemas/user.ts';

export const serverChatMessageSchema = z.looseObject({
  uuid: z.string(),
  user: userSchema.nullable(),
  message: z.string(),
  created: z.coerce.date(),
});

export const serverChatMessageCreateSchema = z.object({
  message: z.string().min(1).max(2000),
});
