// plugged.host: the server's subdomains page (docs/features/subdomains.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSubdomainsSchema } from '@/lib/schemas/server/subdomains.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverSubdomainsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/subdomains`);
  return parseFromApi(serverSubdomainsSchema, data);
};
