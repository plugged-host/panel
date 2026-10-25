// plugged.host: Theme v2's notification button, the sidebar row under the search that opens the inbox
// (docs/features/notifications.md)
import { faBell } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import getUnreadNotifications from '@/api/me/notifications/getUnreadNotifications.ts';
import Button from '@/elements/buttons/Button.tsx';
import NotificationsDrawer from '@/elements/notifications/NotificationsDrawer.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

/** How often the unread count is asked for, like Filament's bell (30 seconds). */
export const NOTIFICATIONS_POLL_MS = 30_000;

export default function SidebarNotifications() {
  const { t } = useTranslations();
  const [opened, setOpened] = useState(false);

  const { data: unread = 0 } = useQuery({
    queryKey: queryKeys.user.notifications.unread(),
    queryFn: getUnreadNotifications,
    refetchInterval: NOTIFICATIONS_POLL_MS,
  });

  const title = t('elements.notifications.title', {});
  const label = unread > 0 ? t('elements.notifications.open', { count: unread }) : title;

  return (
    <>
      <Button
        variant='subtle'
        color='gray'
        fullWidth
        className='theme-sidebar-search theme-sidebar-notifications'
        leftSection={<FontAwesomeIcon icon={faBell} />}
        rightSection={
          unread > 0 ? (
            <span className='theme-notifications-badge' aria-hidden>
              {unread > 99 ? '99+' : unread}
            </span>
          ) : undefined
        }
        onClick={() => setOpened(true)}
        aria-label={label}
        aria-haspopup='dialog'
        styles={{ label: { width: '100%', color: 'var(--theme-text-nav)' } }}
      >
        {title}
      </Button>

      <NotificationsDrawer opened={opened} onClose={() => setOpened(false)} unread={unread} />
    </>
  );
}
