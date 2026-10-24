// plugged.host: the devbox the signed-in person works in on the server
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverDevboxSchema } from '@/lib/schemas/server/devbox.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverDevboxSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/devbox`);
  return parseFromApi(serverDevboxSchema, data);
};
