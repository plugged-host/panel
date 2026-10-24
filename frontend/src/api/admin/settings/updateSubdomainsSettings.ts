// plugged.host: Admin › Settings › Hosting › Subdomains (docs/features/subdomains.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsSubdomainsSchema } from '@/lib/schemas/admin/settings.ts';

export default async (data: z.infer<typeof adminSettingsSubdomainsSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    subdomains: {
      enabled: data.enabled,
      // empty keeps the stored token
      cloudflare_token: data.cloudflareToken.trim() || undefined,
      default_limit: data.defaultLimit,
      cost_first: data.costFirst,
      cost_second: data.costSecond,
      cost_third: data.costThird,
      cost_more: data.costMore,
      blocked_names: data.blockedNames,
    },
  });
};
