// plugged.host: delete every chat of the account, on every server, with their images
import { axiosInstance } from '@/api/axios.ts';

export default async (): Promise<number> => {
  const { data } = await axiosInstance.delete('/api/client/account/neo');
  return Number(data.deleted) || 0;
};
