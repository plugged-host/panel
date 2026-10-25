// plugged.host: the person's backup mails for the server
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSettingsNotificationsSchema } from '@/lib/schemas/server/settings.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  notifications: z.infer<typeof serverSettingsNotificationsSchema>,
): Promise<z.infer<typeof serverSettingsNotificationsSchema>> => {
  const { data } = await axiosInstance.put(
    `/api/client/servers/${uuid}/settings/notifications`,
    serializeForApi(serverSettingsNotificationsSchema, notifications),
  );
  return parseFromApi(serverSettingsNotificationsSchema, data.notifications);
};
