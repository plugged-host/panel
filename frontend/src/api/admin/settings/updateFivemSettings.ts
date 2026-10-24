// plugged.host: Admin › Settings › Hosting › FiveM (the games section's FiveM fields)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsFivemSchema } from '@/lib/schemas/admin/settings.ts';

export default async (data: z.infer<typeof adminSettingsFivemSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    games: {
      fivem_image: data.fivemImage.trim(),
      fivem_enhanced: data.fivemEnhanced,
    },
  });
};
