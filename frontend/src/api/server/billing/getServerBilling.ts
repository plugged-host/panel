// plugged.host: a paid server's month and renewal (docs/features/dedicated.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverBillingSchema } from '@/lib/schemas/server/billing.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverBillingSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/billing`);
  return parseFromApi(serverBillingSchema, data);
};
