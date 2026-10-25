// plugged.host: the review prompt above the server list (docs/features/reviews.md): the same quiet ask
// for every player whose account is old enough and has a server, closed for a long while (per
// account) by its close button or by following it; never for staff, whom review sites forbid asking.
import { useLocation } from 'react-router';
import { z } from 'zod';
import getServers from '@/api/server/getServers.ts';
import Button from '@/elements/buttons/Button.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { isAdmin } from '@/lib/auth/permissions.ts';
import { openUrl } from '@/lib/network/url.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useUserSetting, useUserSettingsLoaded } from '@/lib/userSettings.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useWelcomeTour } from '@/plugins/useWelcomeTour.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

const HOME_PATHS = ['/', '/all', '/grouped'];
// when the account last closed or followed the prompt (ISO 8601), kept with the account's settings
const CLOSED_KEY = 'dashboard::review_prompt_closed';
const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * Whether the prompt is due: the account is `delayDays` old, and the prompt was never closed or
 * closed at least `intervalDays` ago (0: it never comes back).
 */
export function reviewPromptDue(
  created: Date,
  closed: Date | null,
  now: Date,
  delayDays: number,
  intervalDays: number,
): boolean {
  if (now.getTime() - created.getTime() < delayDays * DAY_MS) return false;
  if (!closed || Number.isNaN(closed.getTime())) return true;
  if (intervalDays <= 0) return false;

  return now.getTime() - closed.getTime() >= intervalDays * DAY_MS;
}

export default function ReviewPrompt() {
  const { t } = useTranslations();
  const { pathname } = useLocation();
  const { user, impersonating } = useAuth();
  const app = useGlobalStore((state) => state.settings.app.name);
  const reviews = useGlobalStore((state) => state.settings.reviews);
  const settingsLoaded = useUserSettingsLoaded();
  const [closed, setClosed] = useUserSetting(CLOSED_KEY, z.string().nullable(), null);
  const { tour } = useWelcomeTour();
  const closedAt = closed ? new Date(closed) : null;

  // the account's own settings must be in, so a prompt closed on another device stays closed
  const eligible =
    !!reviews?.url &&
    !!user &&
    !user.suspended &&
    !impersonating &&
    !isAdmin(user) &&
    settingsLoaded &&
    HOME_PATHS.includes(pathname) &&
    !tour?.panelActive &&
    reviewPromptDue(user.created, closedAt, new Date(), reviews.delayDays, reviews.intervalDays);

  // a server of its own or one shared with it: the account has used the panel
  const { data: servers } = useResource({
    queryKey: [...queryKeys.user.servers.all(), 'review-prompt'],
    queryFn: () => getServers(1, undefined, false, 1),
    enabled: eligible,
    silent: true,
  });

  if (!eligible || !servers || servers.total === 0) {
    return null;
  }

  const close = () => setClosed(new Date().toISOString());

  return (
    <div className='theme-main theme-main-banner'>
      <Notification
        className='theme-callout'
        title={t('pages.account.home.review.title', { app })}
        withCloseButton
        closeButtonProps={{ 'aria-label': t('common.button.close', {}), title: t('common.button.close', {}) }}
        onClose={close}
      >
        <Stack gap='sm' align='flex-start'>
          <span>{t('pages.account.home.review.body', { site: reviews.site })}</span>
          <Button
            size='xs'
            variant='default'
            onClick={() => {
              openUrl(reviews.url);
              close();
            }}
          >
            {t('pages.account.home.review.write', {})}
          </Button>
        </Stack>
      </Notification>
    </div>
  );
}
