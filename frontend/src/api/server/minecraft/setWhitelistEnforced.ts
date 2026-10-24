// plugged.host: turn the whitelist on or off (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

/** Returns whether the change went in live (the console) rather than at the next start. */
export default async (uuid: string, enabled: boolean): Promise<boolean> => {
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/minecraft/players/enforce`, { enabled });
  return data.live;
};
