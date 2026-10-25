// plugged.host: Account › Partners (docs/features/partners.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { partnersSchema } from '@/lib/schemas/user/partners.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof partnersSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/partners');
  return parseFromApi(partnersSchema, data.partners);
};
