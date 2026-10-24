// plugged.host: make an uploaded world archive the loaded world (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, archive: string, keep: boolean): Promise<void> => {
  await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/worlds/import`, { archive, keep });
};
