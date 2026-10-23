import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminDeviceSchema } from '@/lib/schemas/admin/devices.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminDeviceSchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/devices', {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminDeviceSchema, data.devices);
};
