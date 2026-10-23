import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSystemStatisticsSchema } from '@/lib/schemas/admin/system.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (nodeUuid: string): Promise<z.infer<typeof adminSystemStatisticsSchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/nodes/${nodeUuid}/system/stats`);
  return parseFromApi(adminSystemStatisticsSchema, data.stats);
};
