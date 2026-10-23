// plugged.host: what the server may be resized to
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSettingsResizeOptionsSchema } from '@/lib/schemas/server/settings.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverSettingsResizeOptionsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/settings/resize`);
  return parseFromApi(serverSettingsResizeOptionsSchema, data.resize);
};
