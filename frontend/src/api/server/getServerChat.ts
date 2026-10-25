import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverChatMessageSchema } from '@/lib/schemas/server/chat.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  page: number,
  perPage?: number,
): Promise<Pagination<z.infer<typeof serverChatMessageSchema>>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/chat`, {
    params: { page, per_page: perPage },
  });
  return parsePaginationFromApi(serverChatMessageSchema, data.messages);
};
