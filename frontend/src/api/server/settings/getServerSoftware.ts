// plugged.host: the software choices of the server's game, null when there are none
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSettingsSoftwareOptionsSchema } from '@/lib/schemas/server/settings.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverSettingsSoftwareOptionsSchema> | null> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/settings/software`);
  return data.software ? parseFromApi(serverSettingsSoftwareOptionsSchema, data.software) : null;
};
