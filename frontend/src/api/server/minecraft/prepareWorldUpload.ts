// plugged.host: an empty folder for a world archive (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

/** Returns the folder to upload the archive into. */
export default async (uuid: string): Promise<string> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/worlds/upload`);
  return data.directory;
};
