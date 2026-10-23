import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminNodeSchema } from '@/lib/schemas/admin/nodes.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

const nodeDeviceSchema = z.object({
  node: adminNodeSchema,
  created: z.coerce.date(),
});

export default async (
  deviceUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof nodeDeviceSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/devices/${deviceUuid}/nodes`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(nodeDeviceSchema, data.node_devices);
};
