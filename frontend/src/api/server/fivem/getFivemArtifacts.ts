// plugged.host: the FXServer builds Cfx offers and what the server runs (docs/features/fivem.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { fivemArtifactsSchema } from '@/lib/schemas/server/fivem.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof fivemArtifactsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/fivem/artifacts`);
  return parseFromApi(fivemArtifactsSchema, data);
};
