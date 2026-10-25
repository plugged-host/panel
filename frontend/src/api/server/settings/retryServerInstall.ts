// plugged.host: starts an install that is stuck over (docs/features/server-settings.md#stuck-installs)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.post(`/api/client/servers/${uuid}/settings/install/retry`);
};
