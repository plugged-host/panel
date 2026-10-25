// plugged.host: delete parts of a player's saved data (docs/features/minecraft.md#player-details)
import { axiosInstance } from '@/api/axios.ts';

export interface PlayerDataRemoval {
  xp: boolean;
  inventory: boolean;
  enderChest: boolean;
  playerData: boolean;
  statistics: boolean;
  advancements: boolean;
}

export default async (uuid: string, player: string, removal: PlayerDataRemoval): Promise<void> => {
  await axiosInstance.delete(`/api/client/servers/${uuid}/minecraft/players/${player}/data`, {
    data: {
      xp: removal.xp,
      inventory: removal.inventory,
      ender_chest: removal.enderChest,
      player_data: removal.playerData,
      statistics: removal.statistics,
      advancements: removal.advancements,
    },
  });
};
