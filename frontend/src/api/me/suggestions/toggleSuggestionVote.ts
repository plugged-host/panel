// plugged.host: voting for an idea and taking the vote back (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionSchema } from '@/lib/schemas/user/suggestions.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof suggestionSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/account/suggestions/${uuid}/vote`);
  return parseFromApi(suggestionSchema, data.suggestion);
};
