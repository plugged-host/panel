import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { noprofitAccountResponseSchema } from '@/lib/schemas/noprofit.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

// plugged.host: the noprofit.space account the signed-in user came from, if any.
export default async (): Promise<z.infer<typeof noprofitAccountResponseSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/noprofit');
  return parseFromApi(noprofitAccountResponseSchema, data);
};
