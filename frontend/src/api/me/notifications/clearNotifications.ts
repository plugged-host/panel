// plugged.host: the notification inbox's "Clear" (docs/features/notifications.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (): Promise<void> => {
  await axiosInstance.delete('/api/client/account/notifications');
};
