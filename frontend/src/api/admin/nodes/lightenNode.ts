// plugged.host: queue moves of a node's servers to the other nodes of its location
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminLightenNodeSchema } from '@/lib/schemas/admin/capacity.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (nodeUuid: string, data: z.infer<typeof adminLightenNodeSchema>): Promise<number> => {
  const { data: response } = await axiosInstance.post(
    `/api/admin/nodes/${nodeUuid}/lighten`,
    serializeForApi(adminLightenNodeSchema, data),
  );
  return response.queued;
};
