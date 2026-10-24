// plugged.host: rux's state: what stops it, the moderation bot's connection, its invite links and models
// (docs/features/rux.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ruxStatusSchema } from '@/lib/schemas/admin/rux.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof ruxStatusSchema>> => {
  const { data } = await axiosInstance.get('/api/admin/rux/status');
  return parseFromApi(ruxStatusSchema, data);
};
