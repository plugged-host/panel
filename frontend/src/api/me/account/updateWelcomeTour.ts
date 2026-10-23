// plugged.host: record a step, the end of a part, a skip or a replay of the welcome tour
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { welcomeTourSchema, welcomeTourUpdateSchema } from '@/lib/schemas/user/welcomeTour.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof welcomeTourUpdateSchema>): Promise<z.infer<typeof welcomeTourSchema>> => {
  const { data: response } = await axiosInstance.post(
    '/api/client/account/tour',
    serializeForApi(welcomeTourUpdateSchema, data),
  );
  return parseFromApi(welcomeTourSchema, response.tour);
};
