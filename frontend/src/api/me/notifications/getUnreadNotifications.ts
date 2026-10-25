// plugged.host: the bell's unread count (docs/features/notifications.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (): Promise<number> => {
  const { data } = await axiosInstance.get('/api/client/account/notifications/unread');
  return data.unread;
};
