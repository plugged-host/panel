// plugged.host: a node's failover mode and newest failovers
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminNodeFailoversSchema } from '@/lib/schemas/admin/failover.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (nodeUuid: string): Promise<z.infer<typeof adminNodeFailoversSchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/nodes/${nodeUuid}/failover`);
  return parseFromApi(adminNodeFailoversSchema, data);
};
