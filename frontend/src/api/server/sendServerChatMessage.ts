import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverChatMessageSchema } from '@/lib/schemas/server/chat.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, message: string): Promise<z.infer<typeof serverChatMessageSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/chat`, { message });
  return parseFromApi(serverChatMessageSchema, data.message);
};
