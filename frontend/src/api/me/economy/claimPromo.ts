// plugged.host: the membership popup, when it is due (marked shown on the panel, once per its hours)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userPromoSchema } from '@/lib/schemas/user/promo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof userPromoSchema> | null> => {
  const { data } = await axiosInstance.post('/api/client/account/promo');
  return data.promo ? parseFromApi(userPromoSchema, data.promo) : null;
};
