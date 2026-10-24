// plugged.host: add a player to a list or take them off it (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftPlayerListSchema } from '@/lib/schemas/server/minecraft.ts';

/** Returns whether the change went in live (the console) rather than at the next start. */
export default async (
  uuid: string,
  list: z.infer<typeof minecraftPlayerListSchema>,
  add: boolean,
  name: string,
  reason?: string,
): Promise<boolean> => {
  const url = `/api/client/servers/${uuid}/minecraft/players/${list}`;
  const { data } = add
    ? await axiosInstance.post(url, { name, reason: reason || undefined })
    : await axiosInstance.delete(url, { data: { name } });
  return data.live;
};
