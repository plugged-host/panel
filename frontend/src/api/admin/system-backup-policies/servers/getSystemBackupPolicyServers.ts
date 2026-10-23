import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSystemBackupPolicyServerSchema } from '@/lib/schemas/admin/systemBackupPolicies.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  policyUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminSystemBackupPolicyServerSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/system-backup-policies/${policyUuid}/servers`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminSystemBackupPolicyServerSchema, data.servers);
};
