// plugged.host: one of this place's chats with its messages
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoConversation, neoConversationSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null, conversation: string): Promise<NeoConversation> => {
  const { data } = await axiosInstance.get(`${neoBase(serverUuid)}/conversations/${conversation}`);
  return parseFromApi(neoConversationSchema, data.conversation);
};
