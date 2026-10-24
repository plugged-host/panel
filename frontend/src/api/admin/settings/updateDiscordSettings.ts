// plugged.host: Admin › Settings › Accounts › Discord (docs/features/discord.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsDiscordSchema } from '@/lib/schemas/admin/settings.ts';

export default async (data: z.infer<typeof adminSettingsDiscordSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    discord: {
      enabled: data.enabled,
      guild_id: data.guildId,
      // empty keeps the stored token
      bot_token: data.botToken.trim() || undefined,
      invite_url: data.inviteUrl,
      oauth_provider_uuid: data.oauthProviderUuid,
      tier_cpu: data.tierCpu,
      tier_memory: data.tierMemory,
      tier_disk: data.tierDisk,
      credits_per_invite: data.creditsPerInvite,
      min_account_age_days: data.minAccountAgeDays,
      invite_maturity_hours: data.inviteMaturityHours,
      invite_clawback: data.inviteClawback,
      boost_one_credits: data.boostOneCredits,
      boost_two_credits: data.boostTwoCredits,
      boost_two_role_id: data.boostTwoRoleId,
      subdomains_require_link: data.subdomainsRequireLink,
      subdomains_require_member: data.subdomainsRequireMember,
      rewards_bot_enabled: data.rewardsBotEnabled,
      assistant_bot_enabled: data.assistantBotEnabled,
      assistant_bot_token: data.assistantBotToken.trim() || undefined,
      moderation_bot_enabled: data.moderationBotEnabled,
      moderation_bot_token: data.moderationBotToken.trim() || undefined,
    },
  });
};
