// plugged.host: cancels or resumes a paid server's renewal; resumed after its month ended, it pays now (docs/features/dedicated.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverBillingSchema } from '@/lib/schemas/server/billing.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, renews: boolean): Promise<z.infer<typeof serverBillingSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/billing`, { renews });
  return parseFromApi(serverBillingSchema, data);
};
