import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverSettingsTrashSchema } from '@/lib/schemas/server/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, data: z.infer<typeof serverSettingsTrashSchema>): Promise<void> => {
  await axiosInstance.put(
    `/api/client/servers/${uuid}/settings/trash`,
    serializeForApi(serverSettingsTrashSchema, data),
  );
};
