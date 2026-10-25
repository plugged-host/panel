// plugged.host: the notification inbox (docs/features/notifications.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userNotificationSchema } from '@/lib/schemas/user/notifications.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export interface NotificationsPage {
  notifications: Pagination<z.infer<typeof userNotificationSchema>>;
  unread: number;
}

export default async (page: number, perPage?: number): Promise<NotificationsPage> => {
  const { data } = await axiosInstance.get('/api/client/account/notifications', {
    params: { page, per_page: perPage },
  });

  return {
    notifications: parsePaginationFromApi(userNotificationSchema, data.notifications),
    unread: data.unread,
  };
};
