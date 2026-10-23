// plugged.host: move the server to a node of another (or the same) location
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverTransferRequestSchema, serverTransferSchema } from '@/lib/schemas/server/transfer.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  data: z.infer<typeof serverTransferSchema>,
): Promise<z.infer<typeof serverTransferRequestSchema>> => {
  const { data: response } = await axiosInstance.post(
    `/api/client/servers/${uuid}/transfer`,
    serializeForApi(serverTransferSchema, data),
  );
  return parseFromApi(serverTransferRequestSchema, response.request);
};
