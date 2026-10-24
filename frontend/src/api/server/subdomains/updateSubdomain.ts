// plugged.host: rename a subdomain, or write its records again (docs/features/subdomains.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSubdomainSchema } from '@/lib/schemas/server/subdomains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  subdomainUuid: string,
  name: string | null,
): Promise<z.infer<typeof serverSubdomainSchema>> => {
  const { data } = await axiosInstance.patch(`/api/client/servers/${uuid}/subdomains/${subdomainUuid}`, { name });
  return parseFromApi(serverSubdomainSchema, data.subdomain);
};
