import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminServerSchema } from '@/lib/schemas/admin/servers.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

const serverDeviceSchema = z.object({
  server: adminServerSchema,
  created: z.coerce.date(),
});

export default async (
  deviceUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof serverDeviceSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/devices/${deviceUuid}/servers`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(serverDeviceSchema, data.server_devices);
};
