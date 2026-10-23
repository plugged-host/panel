import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminServerDeviceSchema } from '@/lib/schemas/admin/servers.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  serverUuid: string,
  page: number,
  search?: string,
  perPage = 100,
): Promise<Pagination<z.infer<typeof adminServerDeviceSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/servers/${serverUuid}/devices`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminServerDeviceSchema, data.devices);
};
