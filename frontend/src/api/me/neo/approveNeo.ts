// plugged.host: approve one waiting tool call, or every one of them (no message)
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoStepResponse, neoStepResponseSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null, message: string | null): Promise<NeoStepResponse> => {
  const { data } = await axiosInstance.post(`${neoBase(serverUuid)}/approve`, message ? { message } : {});
  return parseFromApi(neoStepResponseSchema, data);
};
