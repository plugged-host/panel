import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

const sizesSchema = z.record(z.string(), z.number());

export default async (uuid: string, databaseHostUuid: string): Promise<z.infer<typeof sizesSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/databases/hosts/${databaseHostUuid}/sizes`);
  return parseFromApi(sizesSchema, data.sizes);
};
