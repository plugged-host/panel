// plugged.host: stop the devbox
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverDevboxSchema } from '@/lib/schemas/server/devbox.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverDevboxSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/devbox/stop`);
  return parseFromApi(serverDevboxSchema, data);
};
