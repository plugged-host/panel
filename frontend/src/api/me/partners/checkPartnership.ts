// plugged.host: has the bot check a partnership's Discord server now (docs/features/partners.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { partnersSchema } from '@/lib/schemas/user/partners.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (applicationUuid: string): Promise<z.infer<typeof partnersSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/account/partners/${applicationUuid}/check`);
  return parseFromApi(partnersSchema, data.partners);
};
