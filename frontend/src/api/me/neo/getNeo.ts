// plugged.host: neo's chat in a place (a server's, or the account's): why it may be blocked, the
// person's preferences and the open conversation, which the backend creates on first use
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoInfo, neoSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null): Promise<NeoInfo> => {
  const { data } = await axiosInstance.get(neoBase(serverUuid));
  return parseFromApi(neoSchema, data.neo);
};
