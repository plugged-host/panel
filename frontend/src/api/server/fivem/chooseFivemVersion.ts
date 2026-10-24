// plugged.host: follow a channel, stay on the installed build or install a build (docs/features/fivem.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { FivemVersionChoice, fivemOutcomeSchema } from '@/lib/schemas/server/fivem.ts';

export default async (uuid: string, choice: FivemVersionChoice): Promise<z.infer<typeof fivemOutcomeSchema>> => {
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/fivem/artifacts`, choice);
  return fivemOutcomeSchema.parse(data.outcome);
};
