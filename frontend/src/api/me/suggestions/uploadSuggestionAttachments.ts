// plugged.host: concept art and screenshots on an idea (docs/features/suggestions.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { suggestionAttachmentSchema } from '@/lib/schemas/user/suggestions.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

const responseSchema = z.object({ attachments: z.array(suggestionAttachmentSchema) });

export default async (uuid: string, files: File[]): Promise<z.infer<typeof suggestionAttachmentSchema>[]> => {
  const form = new FormData();
  for (const file of files) form.append('file', file);
  const { data } = await axiosInstance.post(`/api/client/account/suggestions/${uuid}/attachments`, form);
  return parseFromApi(responseSchema, data).attachments;
};
