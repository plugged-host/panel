// plugged.host: restore a node's servers (or, as a drill, one of them) on other nodes
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminStartNodeFailoverSchema } from '@/lib/schemas/admin/failover.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (nodeUuid: string, data: z.infer<typeof adminStartNodeFailoverSchema>): Promise<string> => {
  const { data: response } = await axiosInstance.post(
    `/api/admin/nodes/${nodeUuid}/failover`,
    serializeForApi(adminStartNodeFailoverSchema, data),
  );
  return response.failover;
};
