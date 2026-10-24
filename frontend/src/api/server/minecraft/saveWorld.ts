// plugged.host: have a running server write its world to disk (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

/** Returns whether the server was running and saved. */
export default async (uuid: string): Promise<boolean> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/worlds/save`);
  return data.saved;
};
