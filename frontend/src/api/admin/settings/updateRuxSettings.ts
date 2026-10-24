// plugged.host: Admin › Settings › Accounts › rux (docs/features/rux.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsRuxSchema } from '@/lib/schemas/admin/settings.ts';

type RuxValues = z.infer<typeof adminSettingsRuxSchema>;

// while noprofit.space keeps rux's settings, only what it does not keep is sent
export default async (data: RuxValues, managed = false): Promise<void> => {
  // empty keeps the stored secret
  const noprofitSecret = data.noprofitSecret.trim() || undefined;

  await axiosInstance.put('/api/admin/settings', {
    rux: managed
      ? { name: data.name, noprofit_secret: noprofitSecret }
      : {
          name: data.name,
          guild_id: data.guildId,
          giveaway_channel_id: data.giveawayChannelId,
          mod_log_channel_id: data.modLogChannelId,
          trusted_domains: data.trustedDomains,
          servers: data.servers.map((server) => ({
            id: server.id,
            name: server.name,
            about: server.about,
            giveaway_channel_id: server.giveawayChannelId,
            mod_log_channel_id: server.modLogChannelId,
            trusted_domains: server.trustedDomains,
          })),
          timezone: data.timezone,
          locale: data.locale,
          giveaway_max_credits: data.giveawayMaxCredits,
          giveaway_max_winners: data.giveawayMaxWinners,
          giveaway_max_days: data.giveawayMaxDays,
          credit_granters: data.creditGranters,
          credit_max_per_grant: data.creditMaxPerGrant,
          warn_timeout_at: data.warnTimeoutAt,
          warn_timeout: data.warnTimeout,
          warn_ban_at: data.warnBanAt,
          warn_ban: data.warnBan,
          rule_cooldown_seconds: data.ruleCooldownSeconds,
          watch_guidelines: data.watchGuidelines,
          spam_enabled: data.spamEnabled,
          spam_flood_messages: data.spamFloodMessages,
          spam_flood_seconds: data.spamFloodSeconds,
          spam_duplicates: data.spamDuplicates,
          spam_duplicate_seconds: data.spamDuplicateSeconds,
          spam_mentions: data.spamMentions,
          spam_emoji: data.spamEmoji,
          messages_per_minute: data.messagesPerMinute,
          max_steps: data.maxSteps,
          history_minutes: data.historyMinutes,
          noprofit_secret: noprofitSecret,
        },
  });
};
