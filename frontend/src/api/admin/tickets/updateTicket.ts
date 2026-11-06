// plugged.host: an admin changes a ticket's status or priority (docs/features/tickets.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ticketSchema } from '@/lib/schemas/user/tickets.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  payload: { status?: string; priority?: string },
): Promise<z.infer<typeof ticketSchema>> => {
  const { data } = await axiosInstance.patch(`/api/admin/tickets/${uuid}`, payload);
  return parseFromApi(ticketSchema, data);
};
