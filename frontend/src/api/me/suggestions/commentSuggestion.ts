// plugged.host: commenting on an idea (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionThreadSchema } from '@/lib/schemas/user/suggestions.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, body: string): Promise<z.infer<typeof suggestionThreadSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/account/suggestions/${uuid}/comments`, { body });
  return parseFromApi(suggestionThreadSchema, data);
};
