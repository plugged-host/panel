// plugged.host: where the signed-in user is in the welcome tour
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { welcomeTourSchema } from '@/lib/schemas/user/welcomeTour.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof welcomeTourSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/tour');
  return parseFromApi(welcomeTourSchema, data.tour);
};
