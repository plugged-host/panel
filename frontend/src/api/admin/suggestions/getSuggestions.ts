// plugged.host: Admin › Suggestions (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionSchema } from '@/lib/schemas/user/suggestions.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
  status?: string | null,
  serverType?: string | null,
): Promise<Pagination<z.infer<typeof suggestionSchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/suggestions', {
    params: {
      page,
      per_page: perPage,
      search,
      status: status ?? undefined,
      server_type: serverType ?? undefined,
    },
  });
  return parsePaginationFromApi(suggestionSchema, data.suggestions);
};
