// plugged.host: this place's chats, newest first
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoHistoryEntry, neoHistoryEntrySchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null): Promise<NeoHistoryEntry[]> => {
  const { data } = await axiosInstance.get(`${neoBase(serverUuid)}/conversations`);
  return parseFromApi(z.array(neoHistoryEntrySchema), data.conversations);
};
