// plugged.host: put the software (or modpack) picked in a new Minecraft server's setup in place at once (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';
import { MinecraftSetup } from './completeMinecraftOnboarding.ts';

export type MinecraftSoftwarePick = Pick<MinecraftSetup, 'variant' | 'version' | 'modpack'>;

export default async (uuid: string, pick: MinecraftSoftwarePick): Promise<void> => {
  await axiosInstance.put(`/api/client/servers/${uuid}/minecraft/onboarding/software`, {
    variant: pick.variant,
    version: pick.version,
    modpack: pick.modpack,
  });
};
