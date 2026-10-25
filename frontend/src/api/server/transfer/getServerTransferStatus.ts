// plugged.host: the move the strip on every server page shows, or null
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverTransferStatusSchema } from '@/lib/schemas/server/transfer.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverTransferStatusSchema> | null> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/transfer/status`);
  return data.status ? parseFromApi(serverTransferStatusSchema, data.status) : null;
};
