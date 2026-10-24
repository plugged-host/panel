// plugged.host: start a turn with the person's message, its images (data URLs) and an optional
// seeded read tool (the log check's "Ask neo")
import { axiosInstance } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoSeed, NeoStepResponse, neoStepResponseSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  serverUuid: string | null,
  message: { text: string; images: string[]; seed?: NeoSeed | null },
): Promise<NeoStepResponse> => {
  // sent as it is: the seed's arguments are the tool's and must not be re-cased
  const { data } = await axiosInstance.post(`${neoBase(serverUuid)}/messages`, {
    text: message.text,
    images: message.images,
    seed: message.seed ?? undefined,
  });
  return parseFromApi(neoStepResponseSchema, data);
};
