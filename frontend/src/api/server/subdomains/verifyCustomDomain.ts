// plugged.host: checking a custom domain's CNAME and TXT records (docs/features/subdomains.md#custom-domains)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverCustomDomainSchema } from '@/lib/schemas/server/subdomains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string, customUuid: string): Promise<z.infer<typeof serverCustomDomainSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${serverUuid}/subdomains/custom/${customUuid}/verify`);
  return parseFromApi(serverCustomDomainSchema, data.customDomain);
};
