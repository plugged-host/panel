// plugged.host: a server's subdomains in Admin › Servers (docs/features/subdomains.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSubdomainSchema } from '@/lib/schemas/server/subdomains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export const adminServerSubdomainsSchema = z.object({
  subdomains: z.array(serverSubdomainSchema),
  limit: z.number().nullable(),
  defaultLimit: z.number(),
});

export default async (serverUuid: string): Promise<z.infer<typeof adminServerSubdomainsSchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/servers/${serverUuid}/subdomains`);
  return parseFromApi(adminServerSubdomainsSchema, data);
};
