// plugged.host: restore a parked server, home or to a location
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverLimboSchema } from '@/lib/schemas/server/limbo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, locationUuid: string | null): Promise<z.infer<typeof serverLimboSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/limbo`, {
    location_uuid: locationUuid,
  });
  return parseFromApi(serverLimboSchema, data.limbo);
};
