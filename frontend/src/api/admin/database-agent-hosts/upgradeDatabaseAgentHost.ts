import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminUpgradeResultSchema, adminUpgradeSchema } from '@/lib/schemas/admin/system.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (
  hostUuid: string,
  data: z.infer<typeof adminUpgradeSchema>,
): Promise<z.infer<typeof adminUpgradeResultSchema>> => {
  const { data: response } = await axiosInstance.post(
    `/api/admin/database-agent-hosts/${hostUuid}/upgrade`,
    serializeForApi(adminUpgradeSchema, data),
  );
  return parseFromApi(adminUpgradeResultSchema, response);
};
