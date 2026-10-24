// plugged.host: a one-time link that opens the devbox's VS Code in the browser
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<string> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/devbox/browser`);
  return data.url;
};
