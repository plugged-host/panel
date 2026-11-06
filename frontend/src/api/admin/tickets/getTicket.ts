// plugged.host: one ticket with its thread in Admin › Tickets (docs/features/tickets.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ticketSchema } from '@/lib/schemas/user/tickets.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof ticketSchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/tickets/${uuid}`);
  return parseFromApi(ticketSchema, data);
};
