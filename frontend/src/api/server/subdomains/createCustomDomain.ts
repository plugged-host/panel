// plugged.host: attaching a custom domain to a server (docs/features/subdomains.md#custom-domains)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverCustomDomainSchema } from '@/lib/schemas/server/subdomains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string, domain: string): Promise<z.infer<typeof serverCustomDomainSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${serverUuid}/subdomains/custom`, { domain });
  return parseFromApi(serverCustomDomainSchema, data.customDomain);
};
