// plugged.host: a new Minecraft server's setup and how far applying it got (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftOnboardingSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftOnboardingSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/onboarding`);
  return parseFromApi(minecraftOnboardingSchema, data);
};
