// plugged.host: decline one waiting tool call
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoStepResponse, neoStepResponseSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null, message: string): Promise<NeoStepResponse> => {
  const { data } = await axiosInstance.post(`${neoBase(serverUuid)}/deny`, { message });
  return parseFromApi(neoStepResponseSchema, data);
};
