// plugged.host: where the server may move, its cooldown and its last move
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverTransferOptionsSchema } from '@/lib/schemas/server/transfer.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverTransferOptionsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/transfer`);
  return parseFromApi(serverTransferOptionsSchema, data.transfer);
};
