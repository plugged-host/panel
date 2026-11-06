// plugged.host: an admin assigns a ticket to themselves or clears it (docs/features/tickets.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ticketSchema } from '@/lib/schemas/user/tickets.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, assigned: boolean): Promise<z.infer<typeof ticketSchema>> => {
  const { data } = assigned
    ? await axiosInstance.post(`/api/admin/tickets/${uuid}/assign`)
    : await axiosInstance.delete(`/api/admin/tickets/${uuid}/assign`);
  return parseFromApi(ticketSchema, data);
};
