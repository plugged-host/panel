import { type AxiosRequestConfig } from 'axios';
import { z } from 'zod';
import { axiosInstance, msgpackResponseConfig } from '@/api/axios.ts';
import { serverSchema } from '@/lib/schemas/server/server.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  other?: boolean,
  perPage = 26, // plugged.host: the server list's per-page choice
): Promise<Pagination<z.infer<typeof serverSchema>>> => {
  const config: AxiosRequestConfig = {
    ...msgpackResponseConfig,
    params: { page, per_page: perPage, search, other }, // plugged.host
  };
  const { data } = await axiosInstance.get('/api/client/servers', config);
  return parsePaginationFromApi(serverSchema, data.servers);
};
