// plugged.host: the Discord bots' state (shared/src/discord/gateway.rs, docs/features/discord.md)
import { z } from 'zod';

export const discordBotStatusSchema = z.object({
  bot: z.enum(['rewards', 'assistant', 'moderation']),
  phase: z.enum(['connecting', 'ready', 'refused']),
  username: z.string().nullable(),
  reason: z.enum(['token', 'intents', 'closed', 'members_intent']).nullable(),
  since: z.coerce.date(),
});
