// plugged.host: the person's backup mails for the server
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSettingsNotificationsSchema } from '@/lib/schemas/server/settings.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverSettingsNotificationsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/settings/notifications`);
  return parseFromApi(serverSettingsNotificationsSchema, data.notifications);
};
