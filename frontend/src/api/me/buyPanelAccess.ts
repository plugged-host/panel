import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { panelAccessSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof panelAccessSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/panel-access');
  return parseFromApi(panelAccessSchema, data.panel_access);
};
