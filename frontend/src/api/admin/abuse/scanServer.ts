// plugged.host: Admin › Abuse
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminAbuseScanResultSchema } from '@/lib/schemas/admin/abuse.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string): Promise<z.infer<typeof adminAbuseScanResultSchema>> => {
  const { data } = await axiosInstance.post('/api/admin/abuse/scan', { server_uuid: serverUuid });
  return parseFromApi(adminAbuseScanResultSchema, data);
};
