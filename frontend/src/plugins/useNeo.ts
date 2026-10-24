// plugged.host: where neo's chat is offered, what it is called and what it says in words
// (docs/features/neo.md)
import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { matchPath, useLocation } from 'react-router';
import getNeoUsage from '@/api/me/neo/getNeoUsage.ts';
import { formatTimestamp } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import type { NeoWindow } from '@/lib/schemas/user/neo.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { NEO_ACCOUNT_PLACE, NeoPlace } from '@/stores/neo.ts';
import { useServerStore } from '@/stores/server.ts';

/** The assistant's name as people see it. */
export function useNeoName(): string {
  const { t } = useTranslations();

  return t('elements.neo.name', {});
}

/**
 * The chat of the page the person is on: on a server's pages that server's chat, when they may use
 * neo there (its owner, or a subuser with neo.access, like the backend's `may_use_on`); elsewhere in
 * the dashboard the account's chat. Null where there is none: neo off on the panel, signed out, a
 * suspended account, the admin area, a server they may not use neo on, the waiting room.
 */
export function useNeoPlace(): NeoPlace | null {
  const { user } = useAuth();
  const offered = useGlobalStore((state) => !!state.settings.neo?.enabled);
  const { pathname } = useLocation();
  const server = useServerStore((state) => state.server);
  const canServer = useServerCan('neo.access');

  const serverMatch = matchPath({ path: '/server/:id/*' }, pathname);
  const serverId = serverMatch?.params.id ?? null;
  const onServer = serverId !== null && !!server?.uuid && (server.uuid === serverId || server.uuidShort === serverId);
  // an admin is sent every permission ("*"), so only the subuser's own grant counts here
  const mayUseServer =
    onServer && canServer && (server.isOwner || (!user?.admin && !!server.permissions?.includes('neo.access')));
  const waiting = !!matchPath({ path: '/server/:id/waiting' }, pathname);
  const elsewhere = ['/admin', '/auth', '/oobe'].some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  );

  const serverUuid = mayUseServer && !waiting ? server.uuid : null;
  const serverName = serverUuid ? server.name : null;
  const account = !serverMatch && !elsewhere;
  const available = offered && !!user && !user.suspended;

  return useMemo(() => {
    if (!available) return null;
    if (serverUuid) return { key: serverUuid, serverUuid, serverName };
    return account ? NEO_ACCOUNT_PLACE : null;
  }, [available, serverUuid, serverName, account]);
}

/** The person's limits and the models they can pick (null while neo cannot tell). */
export function useNeoUsage(enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.user.neo.usage(),
    queryFn: getNeoUsage,
    enabled,
    staleTime: 15_000,
    retry: false,
  });
}

const DAY = 86_400;

/** What the chat says in words: notices by their code, plans, windows by their length, times. */
export function useNeoText() {
  const { t } = useTranslations();
  const name = useNeoName();
  const economy = useGlobalStore((state) => state.settings.economy);

  return useMemo(() => {
    /** A moment as the app says it ("in 3 hours"), or "soon" when there is none. */
    const when = (value: unknown): string =>
      typeof value === 'string' && value !== '' && !Number.isNaN(Date.parse(value))
        ? formatTimestamp(value)
        : t('elements.neo.notice.soon', {});

    /** A membership by the access a model needs (`plus`, `plusplus`). */
    const plan = (access: unknown): string | null =>
      access === 'plusplus' ? economy.plusplusName : access === 'plus' ? economy.plusName : null;

    const notice = (code: string | null, params: Record<string, unknown> | null): string => {
      const values = params ?? {};
      const time = when(values.retry_at);

      switch (code) {
        case 'limited_short':
          return t('elements.neo.notice.limitedShort', { name, time });
        case 'limited_long':
          return t('elements.neo.notice.limitedLong', { name, time });
        case 'limited_model':
          return t('elements.neo.notice.limitedModel', { time });
        case 'limited_images':
          return t('elements.neo.notice.limitedImages', { time });
        case 'model_not_allowed':
          return t('elements.neo.notice.modelNotAllowed', {
            model:
              typeof values.model === 'string' && values.model !== ''
                ? values.model
                : t('elements.neo.notice.model', {}),
            plan: plan(values.access) ?? t('elements.neo.notice.plan', {}),
          });
        case 'model_unavailable':
          return t('elements.neo.notice.modelUnavailable', {});
        case 'unlinked':
          return t('elements.neo.notice.unlinked', { name });
        case 'refused':
          return t('elements.neo.notice.refused', { name });
        case 'rate_limited':
          return t('elements.neo.notice.rateLimited', {});
        case 'connection':
          return t('elements.neo.notice.connection', { name });
        case 'disabled':
        case 'turned_off':
          return t('elements.neo.notice.disabled', { name });
        case 'consent':
          return t('elements.neo.notice.consent', {});
        case 'no_access':
          return t('elements.neo.notice.noAccess', {});
        case 'not_yours':
          return t('elements.neo.notice.notYours', {});
        default:
          return t('elements.neo.notice.unavailable', { name });
      }
    };

    /** A window by its length: "5-hour window", "Weekly" ("5h", "Week" when `compact`). */
    const windowLabel = (window: NeoWindow, compact = false): string => {
      const seconds = window.windowSeconds;
      if (seconds <= 0) {
        return t('elements.neo.settings.usage', {});
      }

      if (seconds % DAY === 0) {
        const days = seconds / DAY;
        if (days === 7) {
          return compact ? t('elements.neo.meter.week', {}) : t('elements.neo.settings.weekly', {});
        }

        return compact ? t('elements.neo.meter.days', { days }) : t('elements.neo.settings.days', { days });
      }

      const hours = Math.max(1, Math.round(seconds / 3600));
      return compact ? t('elements.neo.meter.hours', { hours }) : t('elements.neo.settings.hours', { hours });
    };

    /** When a window empties again: "resets in 3 hours", or "not started". */
    const resets = (window: NeoWindow): string =>
      window.resetsAt
        ? t('elements.neo.settings.resets', { time: when(window.resetsAt) })
        : t('elements.neo.settings.notStarted', {});

    return { name, when, plan, notice, windowLabel, resets };
  }, [t, name, economy]);
}
