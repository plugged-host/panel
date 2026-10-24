// plugged.host: switch between FiveM and FiveM for GTAV Enhanced (docs/features/fivem.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { fivemEditionSchema, fivemOutcomeSchema } from '@/lib/schemas/server/fivem.ts';

export default async (
  uuid: string,
  edition: z.infer<typeof fivemEditionSchema>,
): Promise<z.infer<typeof fivemOutcomeSchema>> => {
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/fivem/artifacts/edition`, { edition });
  return fivemOutcomeSchema.parse(data.outcome);
};
