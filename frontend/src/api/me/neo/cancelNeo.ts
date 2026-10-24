// plugged.host: stop the current request: nothing still waiting runs
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoStepResponse, neoStepResponseSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null): Promise<NeoStepResponse> => {
  const { data } = await axiosInstance.post(`${neoBase(serverUuid)}/cancel`, {});
  return parseFromApi(neoStepResponseSchema, data);
};
