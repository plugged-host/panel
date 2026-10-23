import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminEggRepositorySchema } from '@/lib/schemas/admin/eggRepositories.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminEggRepositorySchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/egg-repositories', {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminEggRepositorySchema, data.egg_repositories);
};
