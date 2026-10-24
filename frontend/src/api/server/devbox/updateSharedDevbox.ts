// plugged.host: the owner switches the shared devbox on or off, or changes what it may do
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverDevboxSchema, serverDevboxSharedRightsSchema } from '@/lib/schemas/server/devbox.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  update: { enabled?: boolean; rights?: z.infer<typeof serverDevboxSharedRightsSchema> },
): Promise<z.infer<typeof serverDevboxSchema>> => {
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/devbox/shared`, update);
  return parseFromApi(serverDevboxSchema, data);
};
