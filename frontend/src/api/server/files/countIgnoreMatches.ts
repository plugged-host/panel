import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

const countIgnoreMatchesResponseSchema = z.object({
  counts: z.number().nullable().array(),
});

export default async (uuid: string, patterns: string[], signal?: AbortSignal): Promise<(number | null)[]> => {
  const { data } = await axiosInstance.post(
    `/api/client/servers/${uuid}/files/ignore-matches`,
    { patterns },
    { signal },
  );
  return parseFromApi(countIgnoreMatchesResponseSchema, data).counts;
};
