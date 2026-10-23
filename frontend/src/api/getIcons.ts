import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { iconSchema, iconsSchema } from '@/lib/schemas/icons.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<Record<string, z.infer<typeof iconSchema>>> => {
  const { data } = await axiosInstance.get('/api/icons', { adapter: 'fetch' });
  return parseFromApi(iconsSchema, data).icons;
};
