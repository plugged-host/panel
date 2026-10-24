// plugged.host: submit a new Minecraft server's setup, applied once its install is over (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftSourceSchema } from '@/lib/schemas/server/minecraft.ts';

export interface MinecraftSetup {
  variant: string | null;
  version: string | null;
  modpack: { source: z.infer<typeof minecraftSourceSchema>; project: string; version: string } | null;
  bedrock: boolean;
  onlineMode: boolean;
  private: boolean;
  players: string[];
  operators: string[];
}

export default async (uuid: string, setup: MinecraftSetup): Promise<void> => {
  await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/onboarding`, {
    variant: setup.variant,
    version: setup.version,
    modpack: setup.modpack,
    bedrock: setup.bedrock,
    online_mode: setup.onlineMode,
    private: setup.private,
    players: setup.players,
    operators: setup.operators,
  });
};
