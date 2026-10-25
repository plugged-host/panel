// plugged.host: the notification inbox (docs/features/notifications.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (notificationUuid: string): Promise<void> => {
  await axiosInstance.post(`/api/client/account/notifications/${notificationUuid}/read`);
};
