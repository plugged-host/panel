// plugged.host: try again after an outage or once a limit has reset
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoStepResponse, neoStepResponseSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (serverUuid: string | null): Promise<NeoStepResponse> => {
  const { data } = await axiosInstance.post(`${neoBase(serverUuid)}/retry`, {});
  return parseFromApi(neoStepResponseSchema, data);
};
