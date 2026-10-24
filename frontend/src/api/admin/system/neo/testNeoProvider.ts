// plugged.host: one call to a saved model slot of neo (the Test of Admin › Settings › AI › neo)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminNeoProviderTestSchema } from '@/lib/schemas/admin/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (slot: number): Promise<z.infer<typeof adminNeoProviderTestSchema>> => {
  const { data } = await axiosInstance.post('/api/admin/system/neo/test', { slot });
  return parseFromApi(adminNeoProviderTestSchema, data);
};
