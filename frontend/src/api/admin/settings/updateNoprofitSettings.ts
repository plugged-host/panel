// plugged.host: Admin › Settings › Accounts › noprofit.space
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsNoprofitSchema } from '@/lib/schemas/admin/settings.ts';
import { formExtensionSchemas, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsNoprofitSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    noprofit: serializeForApi(adminSettingsNoprofitSchema, data, formExtensionSchemas('admin.settings.noprofit')),
  });
};
