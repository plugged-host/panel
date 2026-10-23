import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSystemBackupPolicySchema } from '@/lib/schemas/admin/systemBackupPolicies.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminSystemBackupPolicySchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/system-backup-policies', {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminSystemBackupPolicySchema, data.system_backup_policies);
};
