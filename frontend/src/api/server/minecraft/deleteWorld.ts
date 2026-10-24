// plugged.host: delete a world that is not the loaded one (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, world: string): Promise<void> => {
  await axiosInstance.delete(`/api/client/servers/${uuid}/minecraft/worlds`, { data: { world } });
};
