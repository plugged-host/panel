// plugged.host: the notification inbox, Filament's database-notifications slide-over as Theme v2 shows
// it: the newest first, unread marked at the edge, "Mark all as read" and "Clear" in the header
// (docs/features/notifications.md)
import { useInfiniteQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import clearNotifications from '@/api/me/notifications/clearNotifications.ts';
import deleteNotification from '@/api/me/notifications/deleteNotification.ts';
import getNotifications from '@/api/me/notifications/getNotifications.ts';
import markAllNotificationsRead from '@/api/me/notifications/markAllNotificationsRead.ts';
import markNotificationRead from '@/api/me/notifications/markNotificationRead.ts';
import Button from '@/elements/buttons/Button.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import { useCloseMobileMenu } from '@/elements/navigation/Sidebar.tsx';
import NotificationRow from '@/elements/notifications/NotificationRow.tsx';
import Drawer from '@/elements/overlays/Drawer.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { userNotificationSchema } from '@/lib/schemas/user/notifications.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Notification = z.infer<typeof userNotificationSchema>;

const PER_PAGE = 25;
/** While open, the list is asked for again this often, like Filament's polling slide-over. */
const POLL_MS = 30_000;

interface Props {
  opened: boolean;
  onClose: () => void;
  /** The bell's count (the list keeps it fresh while open). */
  unread: number;
}

export default function NotificationsDrawer({ opened, onClose, unread }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const closeMobileMenu = useCloseMobileMenu();

  const { data, isLoading, error, fetchNextPage, hasNextPage, isFetchingNextPage } = useInfiniteQuery({
    queryKey: queryKeys.user.notifications.list(),
    queryFn: async ({ pageParam }) => {
      const page = await getNotifications(pageParam, PER_PAGE);
      // the bell's badge follows what the list says
      queryClient.setQueryData(queryKeys.user.notifications.unread(), page.unread);

      return page;
    },
    initialPageParam: 1,
    getNextPageParam: ({ notifications }) =>
      notifications.page * notifications.perPage < notifications.total ? notifications.page + 1 : undefined,
    enabled: opened,
    // opening shows what the bell counted, not a list from before
    staleTime: 0,
    refetchInterval: opened ? POLL_MS : false,
  });

  const notifications = useMemo(() => data?.pages.flatMap((page) => page.notifications.data) ?? [], [data]);

  const run = async (action: () => Promise<void>) => {
    try {
      await action();
    } catch (err) {
      addToast(httpErrorToHuman(err), 'error');
    } finally {
      await queryClient.invalidateQueries({ queryKey: queryKeys.user.notifications.all() });
    }
  };

  const onOpenServer = (notification: Notification) => {
    if (notification.read === null) {
      void run(() => markNotificationRead(notification.uuid));
    }
    onClose();
    closeMobileMenu?.();
  };

  return (
    <Drawer
      position='right'
      opened={opened}
      onClose={onClose}
      size='md'
      title={
        <span className='theme-notifications-heading'>
          {t('elements.notifications.title', {})}
          {unread > 0 && (
            <Badge size='sm' variant='light'>
              {unread}
            </Badge>
          )}
        </span>
      }
      styles={{ body: { padding: 0 } }}
    >
      {notifications.length > 0 && (
        <div className='theme-notifications-actions'>
          {unread > 0 && (
            <Button variant='subtle' size='compact-sm' onClick={() => run(markAllNotificationsRead)}>
              {t('elements.notifications.markAllRead', {})}
            </Button>
          )}
          <Button variant='subtle' color='gray' size='compact-sm' onClick={() => run(clearNotifications)}>
            {t('elements.notifications.clear', {})}
          </Button>
        </div>
      )}

      {isLoading ? (
        <Spinner.Centered />
      ) : error ? (
        <Text c='red' p='md'>
          {httpErrorToHuman(error)}
        </Text>
      ) : notifications.length === 0 ? (
        <EmptyState
          flush
          title={t('elements.notifications.empty', {})}
          description={t('elements.notifications.emptyHint', {})}
        />
      ) : (
        <>
          <div role='list' aria-label={t('elements.notifications.title', {})} className='theme-notifications-list'>
            {notifications.map((notification) => (
              <NotificationRow
                key={notification.uuid}
                notification={notification}
                onOpenServer={onOpenServer}
                onMarkRead={(entry) => run(() => markNotificationRead(entry.uuid))}
                onDelete={(entry) => run(() => deleteNotification(entry.uuid))}
              />
            ))}
          </div>
          {hasNextPage && (
            <div className='theme-notifications-more'>
              <Button
                variant='subtle'
                color='gray'
                size='compact-sm'
                loading={isFetchingNextPage}
                onClick={() => void fetchNextPage()}
              >
                {t('elements.notifications.showMore', {})}
              </Button>
            </div>
          )}
        </>
      )}
    </Drawer>
  );
}
