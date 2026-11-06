// plugged.host: the suggestions board (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionsSchema } from '@/lib/schemas/user/suggestions.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof suggestionsSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/suggestions');
  return parseFromApi(suggestionsSchema, data);
};
