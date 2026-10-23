// plugged.host: resize within the owner's allowance
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSettingsResizeSchema } from '@/lib/schemas/server/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, data: z.infer<typeof serverSettingsResizeSchema>): Promise<void> => {
  await axiosInstance.post(
    `/api/client/servers/${uuid}/settings/resize`,
    serializeForApi(serverSettingsResizeSchema, data),
  );
};
