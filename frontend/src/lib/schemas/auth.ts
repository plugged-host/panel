import { z } from 'zod';
import { usernameSchema } from '@/lib/schemas/rules.ts';
import { nullableString } from '@/lib/serialization/transformers.ts';

export const authForgotPasswordSchema = z.object({
  email: z.email(),
});

export const authRegisterSchema = z.object({
  username: usernameSchema,
  nameFirst: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  nameLast: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  password: z.string().min(8).max(512),
});

export const authResetPasswordSchema = z
  .object({
    password: z.string().min(8).max(512),
    confirmPassword: z.string().min(8).max(512),
  })
  .refine((data) => data.password === data.confirmPassword, {
    message: 'Passwords do not match',
    path: ['confirmPassword'],
  });

export const authUsernameSchema = z.object({
  username: z.string().nonempty(),
});

export const authPasswordSchema = z.object({
  password: z.string().max(512),
});

export const authTotpSchema = z.object({
  code: z.string().min(6).max(10),
});
