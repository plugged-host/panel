// plugged.host: changing an idea's status and changelog entry (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionSchema } from '@/lib/schemas/user/suggestions.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  payload: { status: string; changelog: string | null },
): Promise<z.infer<typeof suggestionSchema>> => {
  const { data } = await axiosInstance.patch(`/api/admin/suggestions/${uuid}`, payload);
  return parseFromApi(suggestionSchema, data.suggestion);
};
