// plugged.host: switch the software and reinstall
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSettingsSoftwareSchema } from '@/lib/schemas/server/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, data: z.infer<typeof serverSettingsSoftwareSchema>): Promise<void> => {
  await axiosInstance.post(
    `/api/client/servers/${uuid}/settings/software`,
    serializeForApi(serverSettingsSoftwareSchema, data),
  );
};
