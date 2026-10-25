// plugged.host: an admin moves a server to the best node of a location, like a player's move (docs/features/transfers.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

const transferLocationSchema = z.object({
  locationUuid: z.string(),
  start: z.boolean(),
});

export default async (serverUuid: string, data: z.infer<typeof transferLocationSchema>): Promise<void> => {
  await axiosInstance.post(
    `/api/admin/servers/${serverUuid}/transfer/location`,
    serializeForApi(transferLocationSchema, data),
  );
};
