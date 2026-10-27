// plugged.host: the startup command with its values filled in, for "Enable Preview"
// (docs/features/server-settings.md#startup); with `command`, that command instead of the saved one
// (a custom one being written, docs/features/startup-commands.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, command?: string): Promise<string> => {
  const { data } =
    command === undefined
      ? await axiosInstance.get(`/api/client/servers/${uuid}/startup/preview`)
      : await axiosInstance.post(`/api/client/servers/${uuid}/startup/preview`, { command });
  return data.command;
};
