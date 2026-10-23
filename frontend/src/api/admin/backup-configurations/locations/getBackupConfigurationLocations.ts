import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminLocationSchema } from '@/lib/schemas/admin/locations.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  backupConfigUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminLocationSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/backup-configurations/${backupConfigUuid}/locations`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminLocationSchema, data.locations);
};
