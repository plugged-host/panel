// plugged.host: the signed-in user's standing (docs/features/standing.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { accountStandingResponseSchema } from '@/lib/schemas/standing.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof accountStandingResponseSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/standing');
  return parseFromApi(accountStandingResponseSchema, data);
};
