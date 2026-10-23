// plugged.host: update an installed plugin or mod to its newest build (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, file: string): Promise<string> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/content/installed/update`, {
    file,
  });
  return data.file;
};
