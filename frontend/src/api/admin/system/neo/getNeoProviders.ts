// plugged.host: where each of neo's model slots stands (Admin › Settings › AI › neo)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminNeoProviderStatusSchema, adminNeoProvidersSchema } from '@/lib/schemas/admin/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof adminNeoProviderStatusSchema>[]> => {
  const { data } = await axiosInstance.get('/api/admin/system/neo/providers');
  return parseFromApi(adminNeoProvidersSchema, data).providers;
};
