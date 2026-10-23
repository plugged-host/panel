// plugged.host: take the server out of the start queue without starting it
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/client/servers/${uuid}/queue`);
};
