import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminNodeServerBackupSchema } from '@/lib/schemas/admin/nodes.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  backupConfigUuid: string,
  page: number,
  search?: string,
  detached?: boolean,
  perPage?: number,
): Promise<{ backups: Pagination<z.infer<typeof adminNodeServerBackupSchema>>; failed: number }> => {
  const { data } = await axiosInstance.get(`/api/admin/backup-configurations/${backupConfigUuid}/backups`, {
    params: { page, per_page: perPage, search, detached },
  });
  return {
    backups: parsePaginationFromApi(adminNodeServerBackupSchema, data.backups),
    failed: data.failed,
  };
};
