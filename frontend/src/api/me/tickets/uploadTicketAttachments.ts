// plugged.host: images and files on a ticket (docs/features/tickets.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ticketAttachmentSchema } from '@/lib/schemas/user/tickets.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

const responseSchema = z.object({ attachments: z.array(ticketAttachmentSchema) });

export default async (uuid: string, files: File[]): Promise<z.infer<typeof ticketAttachmentSchema>[]> => {
  const form = new FormData();
  for (const file of files) form.append('file', file);
  const { data } = await axiosInstance.post(`/api/client/account/tickets/${uuid}/attachments`, form);
  return parseFromApi(responseSchema, data).attachments;
};
