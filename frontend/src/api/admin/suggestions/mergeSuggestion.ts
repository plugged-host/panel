// plugged.host: merging a duplicate idea into another (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionSchema } from '@/lib/schemas/user/suggestions.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, targetUuid: string): Promise<z.infer<typeof suggestionSchema>> => {
  const { data } = await axiosInstance.post(`/api/admin/suggestions/${uuid}/merge`, {
    target_uuid: targetUuid,
  });
  return parseFromApi(suggestionSchema, data.suggestion);
};
