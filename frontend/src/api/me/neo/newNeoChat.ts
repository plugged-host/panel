// plugged.host: put the open chat into the history and start an empty one (409 while neo answers)
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoConversation, neoConversationSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null): Promise<NeoConversation> => {
  const { data } = await axiosInstance.post(`${neoBase(serverUuid)}/new`, {});
  return parseFromApi(neoConversationSchema, data.conversation);
};
