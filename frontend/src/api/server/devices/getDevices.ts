import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverDeviceSchema } from '@/lib/schemas/server/devices.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof serverDeviceSchema>>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/devices`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(serverDeviceSchema, data.devices);
};
