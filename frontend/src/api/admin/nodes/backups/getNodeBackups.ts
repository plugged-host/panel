import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminServerBackupSchema } from '@/lib/schemas/admin/servers.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  nodeUuid: string,
  page: number,
  search?: string,
  detached?: boolean,
  perPage?: number,
): Promise<{ backups: Pagination<z.infer<typeof adminServerBackupSchema>>; failed: number }> => {
  const { data } = await axiosInstance.get(`/api/admin/nodes/${nodeUuid}/backups`, {
    params: { page, per_page: perPage, search, detached },
  });
  return {
    backups: parsePaginationFromApi(adminServerBackupSchema, data.backups),
    failed: data.failed,
  };
};
