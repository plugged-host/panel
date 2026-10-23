import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminEggSchema } from '@/lib/schemas/admin/eggs.ts';
import { adminNestSchema } from '@/lib/schemas/admin/nests.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

const nestEggDeviceSchema = z.object({
  nest: adminNestSchema,
  nestEgg: adminEggSchema,
  created: z.coerce.date(),
});

export default async (
  deviceUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof nestEggDeviceSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/devices/${deviceUuid}/nest-eggs`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(nestEggDeviceSchema, data.nest_egg_devices);
};
