// plugged.host: Admin › Settings › Hosting › Devboxes (docs/features/devboxes.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsDevboxesSchema } from '@/lib/schemas/admin/settings.ts';

export default async (data: z.infer<typeof adminSettingsDevboxesSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    devboxes: {
      enabled: data.enabled,
      egg_uuid: data.eggUuid || null,
      image: data.image,
      members_only: data.membersOnly,
      cpu: data.cpu,
      memory: data.memory,
      disk: data.disk,
      premium_cpu: data.premiumCpu,
      premium_memory: data.premiumMemory,
      premium_disk: data.premiumDisk,
      idle_minutes: data.idleMinutes,
      max_hours: data.maxHours,
      running_per_user: data.runningPerUser,
      cleanup_days: data.cleanupDays,
      panel_url: data.panelUrl,
      web_domain: data.webDomain,
      web_origin_ip: data.webOriginIp,
      // empty keeps the stored token
      cloudflare_token: data.cloudflareToken.trim() || undefined,
    },
  });
};
