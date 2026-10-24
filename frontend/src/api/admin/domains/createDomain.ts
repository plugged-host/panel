// plugged.host: add a domain; the panel finds its Cloudflare zone (docs/features/subdomains.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminDomainSchema } from '@/lib/schemas/admin/domains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (name: string): Promise<z.infer<typeof adminDomainSchema>> => {
  const { data } = await axiosInstance.post('/api/admin/domains', { name });
  return parseFromApi(adminDomainSchema, data.domain);
};
