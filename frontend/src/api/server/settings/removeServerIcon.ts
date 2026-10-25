// plugged.host: deletes the server's icon, so it shows its game's again
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/client/servers/${uuid}/settings/icon`);
};
