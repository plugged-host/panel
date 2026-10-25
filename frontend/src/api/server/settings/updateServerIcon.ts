// plugged.host: uploads the server's icon, like the account avatar (docs/features/server-settings.md#server-icon)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, icon: Blob): Promise<string> => {
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/settings/icon`, icon, {
    headers: {
      'Content-Type': icon.type,
    },
  });
  return data.icon;
};
