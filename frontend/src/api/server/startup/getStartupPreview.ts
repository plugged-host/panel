// plugged.host: the startup command with its values filled in, for "Enable Preview"
// (docs/features/server-settings.md#startup)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<string> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/startup/preview`);
  return data.command;
};
