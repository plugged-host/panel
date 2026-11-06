// plugged.host: the account's tickets (docs/features/tickets.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ticketsSchema } from '@/lib/schemas/user/tickets.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof ticketsSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/tickets');
  return parseFromApi(ticketsSchema, data);
};
