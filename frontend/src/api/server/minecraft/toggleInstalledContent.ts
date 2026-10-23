// plugged.host: turn an installed plugin or mod on or off (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, file: string, enabled: boolean): Promise<string> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/content/installed/toggle`, {
    file,
    enabled,
  });
  return data.file;
};
