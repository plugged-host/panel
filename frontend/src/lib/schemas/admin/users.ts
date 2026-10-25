import { z } from 'zod';
import { oAuthProviderSchema } from '@/lib/schemas/generic.ts';
import { usernameSchema } from '@/lib/schemas/rules.ts';
import { roleSchema, twoFactorMethod } from '@/lib/schemas/user.ts';
import { nullableString } from '@/lib/serialization/transformers.ts';

export const adminUserSchema = z.looseObject({
  uuid: z.string(),
  externalId: z.preprocess(nullableString, z.string().nullable()),
  username: z.string(),
  email: z.email(),
  nameFirst: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  nameLast: z.preprocess(nullableString, z.string().min(1).max(255).nullable()),
  admin: z.boolean(),
  frozen: z.boolean(),
  suspended: z.boolean(),
  language: z.string(),
  role: z.lazy(() => roleSchema).nullable(),
  created: z.coerce.date(),
});

export const adminFullUserSchema = z.lazy(() =>
  adminUserSchema.extend({
    avatar: z.string().nullable(),
    totpEnabled: z.boolean(),
    totpLastUsed: z.coerce.date().nullable(),
    emailTwoFactorEnabled: z.boolean(),
    twoFactorMethods: z.array(z.lazy(() => twoFactorMethod)),
    requireTwoFactor: z.boolean(),
    twoFactorSatisfied: z.boolean(),
    emailVerified: z.boolean(),
    requireEmailVerification: z.boolean(),
    passwordLoginDisabled: z.boolean(),
    hasPassword: z.boolean(),
    // plugged.host: the noprofit.space and Discord accounts it is linked to (shared/src/user_links.rs)
    links: z
      .object({
        noprofit: z.boolean(),
        discord: z.object({ id: z.string(), username: z.string().nullable() }).nullable(),
      })
      .nullable()
      .default(null),
  }),
);

export const adminUserUpdateSchema = z.lazy(() =>
  adminUserSchema
    .omit({
      uuid: true,
      role: true,
      created: true,
    })
    .extend({
      username: usernameSchema,
      roleUuid: z.string().nullable(),
      password: z.preprocess(nullableString, z.string().nullable()),
    }),
);

export const adminUserOAuthLinkSchema = z.looseObject({
  uuid: z.string(),
  oauthProvider: z.lazy(() => oAuthProviderSchema),
  identifier: z.string(),
  lastUsed: z.coerce.date().nullable(),
  created: z.coerce.date(),
});
