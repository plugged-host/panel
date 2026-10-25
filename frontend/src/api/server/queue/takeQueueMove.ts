// plugged.host: take the start queue page's move to a quieter node of the server's location
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.post(`/api/client/servers/${uuid}/queue/transfer`);
};
