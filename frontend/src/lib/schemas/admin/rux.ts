// plugged.host: rux's state for Admin › Settings › Accounts › rux (backend/src/routes/api/admin/rux.rs,
// docs/features/rux.md)
import { z } from 'zod';
import { discordBotStatusSchema } from '@/lib/schemas/admin/discord.ts';
import { adminSettingsRuxStoreSchema } from '@/lib/schemas/admin/settings.ts';

export const ruxBlockerSchema = z.enum([
  'discord_off',
  'bot_off',
  'no_token',
  'no_server',
  'switched_off',
  'no_accounts',
  'no_ai',
]);

export const ruxStatusSchema = z.object({
  settings: adminSettingsRuxStoreSchema,
  managed: z
    .object({
      by: z.string(),
      url: z.string().nullable(),
      fetchedAt: z.number(),
      savedAt: z.string().nullable(),
      savedBy: z.string().nullable(),
    })
    .nullable(),
  guilds: z.array(
    z.object({
      id: z.string(),
      primary: z.boolean(),
      hostName: z.string(),
    }),
  ),
  blockers: z.array(ruxBlockerSchema),
  noprofitConnected: z.boolean(),
  bot: discordBotStatusSchema.nullable(),
  invites: z.array(z.object({ guildId: z.string(), name: z.string(), url: z.string() })),
  aiSource: z.enum(['panel', 'noprofit']),
  orderModels: z.array(z.string()),
  checkModels: z.array(z.string()),
});
