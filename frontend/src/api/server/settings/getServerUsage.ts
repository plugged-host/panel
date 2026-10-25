// plugged.host: what the server uses of its backup, database and allocation limits
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSettingsUsageSchema } from '@/lib/schemas/server/settings.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverSettingsUsageSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/settings/usage`);
  return parseFromApi(serverSettingsUsageSchema, data.usage);
};
