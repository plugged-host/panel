// plugged.host: a player's game mode (docs/features/minecraft.md#player-details)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftGameModeSchema } from '@/lib/schemas/server/minecraft.ts';

/** Returns whether the change went in live (the console) rather than into the player's file. */
export default async (
  uuid: string,
  player: string,
  mode: z.infer<typeof minecraftGameModeSchema>,
): Promise<boolean> => {
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/minecraft/players/${player}/game-mode`, {
    mode,
  });
  return data.live;
};
