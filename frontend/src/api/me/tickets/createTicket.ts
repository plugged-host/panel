// plugged.host: opening a ticket (docs/features/tickets.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ticketSchema } from '@/lib/schemas/user/tickets.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (payload: {
  subject: string;
  category: string;
  serverUuid: string | null;
  body: string;
}): Promise<z.infer<typeof ticketSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/tickets', {
    subject: payload.subject,
    category: payload.category,
    server_uuid: payload.serverUuid,
    body: payload.body,
  });
  return parseFromApi(ticketSchema, data);
};
