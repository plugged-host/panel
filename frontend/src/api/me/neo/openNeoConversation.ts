// plugged.host: continue an earlier chat: it becomes the open one (409 while neo answers)
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoConversation, neoConversationSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null, conversation: string): Promise<NeoConversation> => {
  const { data } = await axiosInstance.post(`${neoBase(serverUuid)}/conversations/${conversation}/open`, {});
  return parseFromApi(neoConversationSchema, data.conversation);
};
