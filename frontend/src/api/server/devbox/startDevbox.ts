// plugged.host: start the devbox (made on its first start)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverDevboxSchema } from '@/lib/schemas/server/devbox.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverDevboxSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/devbox/start`);
  return parseFromApi(serverDevboxSchema, data);
};
