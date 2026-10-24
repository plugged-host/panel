// plugged.host: delete one of this place's chats (409 while neo answers in it)
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';

export default async (serverUuid: string | null, conversation: string): Promise<void> => {
  await axiosInstance.delete(`${neoBase(serverUuid)}/conversations/${conversation}`);
};
