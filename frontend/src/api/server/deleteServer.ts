// plugged.host: an owner deleting their server
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/client/servers/${uuid}`);
};
