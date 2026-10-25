// plugged.host: a failover's dry run: where each of the node's servers would go, nothing moved
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminFailoverPlansSchema } from '@/lib/schemas/admin/failover.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (nodeUuid: string): Promise<z.infer<typeof adminFailoverPlansSchema>['plan']> => {
  const { data } = await axiosInstance.get(`/api/admin/nodes/${nodeUuid}/failover/plan`);
  return parseFromApi(adminFailoverPlansSchema, data).plan;
};
