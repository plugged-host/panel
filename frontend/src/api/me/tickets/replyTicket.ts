// plugged.host: replying to a ticket (docs/features/tickets.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ticketSchema } from '@/lib/schemas/user/tickets.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, body: string): Promise<z.infer<typeof ticketSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/account/tickets/${uuid}/reply`, { body });
  return parseFromApi(ticketSchema, data);
};
