// plugged.host: add a subdomain; the owner pays its tokens (docs/features/subdomains.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSubdomainSchema } from '@/lib/schemas/server/subdomains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  name: string,
  domainUuid: string | null,
): Promise<z.infer<typeof serverSubdomainSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/subdomains`, {
    name,
    domain_uuid: domainUuid,
  });
  return parseFromApi(serverSubdomainSchema, data.subdomain);
};
