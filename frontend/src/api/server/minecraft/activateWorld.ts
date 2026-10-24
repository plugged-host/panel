// plugged.host: load another world from the next start (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, world: string): Promise<void> => {
  await axiosInstance.put(`/api/client/servers/${uuid}/minecraft/worlds/active`, { world });
};
