import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminDatabaseHostDatabaseSizeSchema } from '@/lib/schemas/admin/databaseHosts.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

const sizesSchema = z.array(adminDatabaseHostDatabaseSizeSchema);

export default async (databaseHostUuid: string): Promise<z.infer<typeof sizesSchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/database-hosts/${databaseHostUuid}/databases/sizes`);
  return parseFromApi(sizesSchema, data.databases);
};
