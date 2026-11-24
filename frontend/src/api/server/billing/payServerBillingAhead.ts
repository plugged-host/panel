// plugged.host: pays a paid server's next month now from noprofit.space Funds, or the unpaid one (docs/features/dedicated.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverBillingSchema } from '@/lib/schemas/server/billing.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverBillingSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/billing/pay`);
  return parseFromApi(serverBillingSchema, data);
};
