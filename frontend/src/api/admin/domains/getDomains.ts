// plugged.host: Admin › Domains (docs/features/subdomains.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminDomainsSchema } from '@/lib/schemas/admin/domains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof adminDomainsSchema>> => {
  const { data } = await axiosInstance.get('/api/admin/domains');
  return parseFromApi(adminDomainsSchema, data);
};
