// plugged.host: posting an idea (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionSchema } from '@/lib/schemas/user/suggestions.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (payload: {
  title: string;
  body: string;
  serverType: string;
}): Promise<z.infer<typeof suggestionSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/suggestions', {
    title: payload.title,
    body: payload.body,
    server_type: payload.serverType,
  });
  return parseFromApi(suggestionSchema, data.suggestion);
};
