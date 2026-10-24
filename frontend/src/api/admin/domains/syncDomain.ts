// plugged.host: look a domain's Cloudflare zone up again (docs/features/subdomains.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminDomainSchema } from '@/lib/schemas/admin/domains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof adminDomainSchema>> => {
  const { data } = await axiosInstance.post(`/api/admin/domains/${uuid}/sync`);
  return parseFromApi(adminDomainSchema, data.domain);
};
