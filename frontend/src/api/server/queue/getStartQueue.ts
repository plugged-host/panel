// plugged.host: the server's place in the start queue
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverStartQueueSchema } from '@/lib/schemas/server/queue.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverStartQueueSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/queue`);
  return parseFromApi(serverStartQueueSchema, data.queue);
};
