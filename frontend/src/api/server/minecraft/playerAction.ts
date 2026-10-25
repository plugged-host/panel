// plugged.host: heal, feed or kill an online player (docs/features/minecraft.md#player-details)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftPlayerActionSchema } from '@/lib/schemas/server/minecraft.ts';

export default async (
  uuid: string,
  player: string,
  action: z.infer<typeof minecraftPlayerActionSchema>,
): Promise<void> => {
  await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/players/${player}/action`, { action });
};
