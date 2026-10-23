// plugged.host: Admin › Overview › Capacity: what each location can still take, and the moves
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminCapacitySchema } from '@/lib/schemas/admin/capacity.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof adminCapacitySchema>> => {
  const { data } = await axiosInstance.get('/api/admin/system/capacity');
  return parseFromApi(adminCapacitySchema, data);
};
