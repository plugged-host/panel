import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminUpgradeStatusSchema } from '@/lib/schemas/admin/system.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (hostUuid: string): Promise<z.infer<typeof adminUpgradeStatusSchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/database-agent-hosts/${hostUuid}/upgrade`);
  return parseFromApi(adminUpgradeStatusSchema, data);
};
