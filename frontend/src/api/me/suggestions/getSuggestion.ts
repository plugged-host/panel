// plugged.host: one idea with its discussion and attachments (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionThreadSchema } from '@/lib/schemas/user/suggestions.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof suggestionThreadSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/account/suggestions/${uuid}`);
  return parseFromApi(suggestionThreadSchema, data);
};
