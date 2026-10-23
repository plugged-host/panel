// plugged.host: leave the start waiting room once it is ready
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, blocked: boolean): Promise<void> => {
  await axiosInstance.post(`/api/client/servers/${uuid}/waiting-room`, { blocked });
};
