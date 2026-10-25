// plugged.host: move a player (docs/features/minecraft.md#player-details)
import { axiosInstance } from '@/api/axios.ts';

export type TeleportDestination =
  | { kind: 'coordinates'; x: number; y: number; z: number; dimension: string | null }
  | { kind: 'spawn' }
  | { kind: 'player'; uuid: string };

/** Returns whether the player was moved live (the console) rather than in their file. */
export default async (uuid: string, player: string, destination: TeleportDestination): Promise<boolean> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/players/${player}/teleport`, {
    destination,
  });
  return data.live;
};
