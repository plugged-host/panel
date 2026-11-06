import { z } from 'zod';
import { userSchema } from '@/lib/schemas/user.ts';

export const serverSubuserSchema = z.looseObject({
  user: z.lazy(() => userSchema),
  permissions: z.array(z.string()),
  ignoredFiles: z.array(z.string()),
  created: z.coerce.date(),
});

export const serverSubuserCreateSchema = z.object({
  username: z
    .string()
    .min(3)
    .max(15)
    .regex(/^[a-zA-Z0-9_]+$/),
  permissions: z.array(z.string()),
  ignoredFiles: z.array(z.string()),
});

export const serverSubuserUpdateSchema = z.object({
  permissions: z.array(z.string()),
  ignoredFiles: z.array(z.string()),
});
