// plugged.host: one notification of the inbox, Filament's database notification as Theme v2 shows it
// (docs/features/notifications.md)
import classNames from 'classnames';
import { ReactNode } from 'react';
import { Link } from 'react-router';
import { z } from 'zod';
import CloseButton from '@/elements/buttons/CloseButton.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import RedactedText from '@/elements/typography/RedactedText.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { formatDateTime } from '@/lib/format/time.ts';
import { intlLocale } from '@/lib/languages.ts';
import { userNotificationSchema } from '@/lib/schemas/user/notifications.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Notification = z.infer<typeof userNotificationSchema>;
type Translations = ReturnType<typeof useTranslations>;
type Data = Record<string, unknown>;

/** The words a notification shows: a title, a sentence or two, and lines of detail. */
interface NotificationText {
  title: string;
  body: string | null;
  details: ReactNode[];
  /** Where its server link goes and what it says, when not the server's console ("Open server"). */
  serverPage?: { path: string; label: string };
}

/** The `pages.server.transfer.reasons` a finished move can name (`server_transfers::reason_key`). */
const TRANSFER_REASONS = ['noNode', 'copyFailed', 'skippedMoved'] as const;

const record = (value: unknown): Data =>
  value && typeof value === 'object' && !Array.isArray(value) ? (value as Data) : {};
const text = (value: unknown): string =>
  typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '';
const count = (value: unknown): number => (typeof value === 'number' && Number.isFinite(value) ? value : 0);

function transferReason({ t }: Translations, value: unknown): string {
  const reason = TRANSFER_REASONS.find((known) => known === value);

  return reason ? t(`pages.server.transfer.reasons.${reason}`, {}) : '';
}

/** Draws a notification's text from its kind and data, in the reader's language. */
export function notificationText(translations: Translations, notification: Notification): NotificationText {
  const { t, tReact, language } = translations;
  const data = record(notification.data);
  const server = text(data.server);
  const location = text(data.location);
  const list = new Intl.ListFormat(intlLocale(language), { style: 'long', type: 'conjunction' });

  const address = (value: unknown): ReactNode[] =>
    text(value)
      ? [
          tReact('elements.notifications.address', {
            address: (
              <span dir='ltr'>
                <RedactedText value={text(value)} />
              </span>
            ),
          }),
        ]
      : [];
  const reason = (value: unknown): ReactNode[] => {
    const words = transferReason(translations, value);

    return words ? [t('elements.notifications.reason', { reason: words })] : [];
  };
  const error = (value: unknown): ReactNode[] => (text(value) ? [text(value)] : []);
  // plugged.host: the partner program's words (docs/features/partners.md)
  const guild = text(data.guild);
  const credits = (value: unknown): string => count(value).toLocaleString(intlLocale(language));
  const because = (value: unknown): ReactNode[] =>
    text(value) ? [t('elements.notifications.reason', { reason: text(value) })] : [];
  const day = (value: unknown): string => {
    const date = new Date(text(value));

    return Number.isNaN(date.getTime())
      ? ''
      : date.toLocaleDateString(intlLocale(language), { year: 'numeric', month: 'long', day: 'numeric' });
  };

  switch (notification.kind) {
    case 'transferCompleted':
      return {
        title: t('elements.notifications.kinds.transferCompleted.title', {}),
        body: location
          ? t('elements.notifications.kinds.transferCompleted.body', { server, location })
          : t('elements.notifications.kinds.transferCompleted.bodyAnywhere', { server }),
        details: address(data.address),
      };
    case 'transferFailed':
      return {
        title: t('elements.notifications.kinds.transferFailed.title', {}),
        body: location
          ? t('elements.notifications.kinds.transferFailed.body', { server, location })
          : t('elements.notifications.kinds.transferFailed.bodyAnywhere', { server }),
        details: reason(data.reason),
      };
    case 'admin.transferFailed': {
      const why = reason(data.reason);

      return {
        title: t('elements.notifications.kinds.admin.transferFailed.title', { server }),
        body: location
          ? t('elements.notifications.kinds.admin.transferFailed.body', { server, location })
          : t('elements.notifications.kinds.admin.transferFailed.bodyAnywhere', { server }),
        details: why.length > 0 ? why : error(data.error),
      };
    }
    case 'allowanceCut': {
      const parts = new Intl.ListFormat(intlLocale(language), { style: 'short', type: 'unit' });
      const servers = (Array.isArray(data.servers) ? data.servers : []).map((entry) => {
        const cut = record(entry);
        const amounts = [
          count(cut.cpu) > 0 && t('elements.notifications.kinds.allowanceCut.cpu', { amount: count(cut.cpu) }),
          count(cut.memory) > 0 &&
            t('elements.notifications.kinds.allowanceCut.memory', {
              amount: bytesToString(mbToBytes(count(cut.memory)), 1),
            }),
          count(cut.disk) > 0 &&
            t('elements.notifications.kinds.allowanceCut.disk', {
              amount: bytesToString(mbToBytes(count(cut.disk)), 1),
            }),
        ].filter((amount): amount is string => !!amount);

        return amounts.length > 0 ? `${text(cut.name)} (${parts.format(amounts)})` : text(cut.name);
      });

      return {
        title: t('elements.notifications.kinds.allowanceCut.title', {}),
        body: t('elements.notifications.kinds.allowanceCut.body', { servers: list.format(servers) }),
        details: [],
      };
    }
    case 'membershipEnded':
      return {
        title: t('elements.notifications.kinds.membershipEnded.title', { membership: text(data.membership) }),
        body: t('elements.notifications.kinds.membershipEnded.body', {
          servers: list.format((Array.isArray(data.servers) ? data.servers : []).map(text)),
        }),
        details: [],
      };
    case 'failoverDone': {
      const drill = data.drill === true;
      const backup = text(data.backup);

      return {
        title: drill
          ? t('elements.notifications.kinds.failoverDone.drillTitle', {})
          : t('elements.notifications.kinds.failoverDone.title', {}),
        body: drill
          ? t('elements.notifications.kinds.failoverDone.drillBody', { server })
          : t('elements.notifications.kinds.failoverDone.body', { server }),
        details: [
          ...address(data.address),
          ...(backup ? [t('elements.notifications.kinds.failoverDone.backup', { time: formatDateTime(backup) })] : []),
        ],
      };
    }
    case 'failoverFailed':
      return {
        title: t('elements.notifications.kinds.failoverFailed.title', {}),
        body: t('elements.notifications.kinds.failoverFailed.body', { server }),
        details: [],
      };
    case 'modpackInstalled':
      return {
        title: t('elements.notifications.kinds.modpackInstalled.title', {}),
        body: t('elements.notifications.kinds.modpackInstalled.body', {
          name: text(data.name),
          server,
          count: count(data.files),
        }),
        details: count(data.skipped) > 0 ? [t('elements.notifications.kinds.modpackInstalled.skipped', {})] : [],
      };
    case 'modpackFailed':
      return {
        title: t('elements.notifications.kinds.modpackFailed.title', {}),
        body: t('elements.notifications.kinds.modpackFailed.body', { name: text(data.name), server }),
        details: error(data.error),
      };
    case 'frameworkInstalled':
      return {
        title: t('elements.notifications.kinds.frameworkInstalled.title', { recipe: text(data.recipe), server }),
        body: text(data.folder)
          ? t('elements.notifications.kinds.frameworkInstalled.body', { folder: text(data.folder) })
          : null,
        details: [],
      };
    case 'frameworkFailed':
      return {
        title: t('elements.notifications.kinds.frameworkFailed.title', { recipe: text(data.recipe), server }),
        body: null,
        details: error(data.error),
      };
    case 'worldImported':
      return {
        title: t('elements.notifications.kinds.worldImported.title', {}),
        body: t('elements.notifications.kinds.worldImported.body', { server }),
        details: text(data.kept)
          ? [t('elements.notifications.kinds.worldImported.kept', { world: text(data.kept) })]
          : [],
      };
    case 'worldImportFailed':
      return {
        title: t('elements.notifications.kinds.worldImportFailed.title', {}),
        body: t('elements.notifications.kinds.worldImportFailed.body', { server }),
        details: error(data.error),
      };
    case 'serverInstalled':
      return {
        title: t('elements.notifications.kinds.serverInstalled.title', {}),
        body: t('elements.notifications.kinds.serverInstalled.body', { server }),
        details: [],
      };
    case 'serverInstallFailed':
      return {
        title: t('elements.notifications.kinds.serverInstallFailed.title', {}),
        body: t('elements.notifications.kinds.serverInstallFailed.body', { server }),
        details: [],
      };
    case 'subuserAdded':
      return {
        title: t('elements.notifications.kinds.subuserAdded.title', {}),
        body: t('elements.notifications.kinds.subuserAdded.body', { server }),
        details: [],
      };
    case 'subuserRemoved':
      return {
        title: t('elements.notifications.kinds.subuserRemoved.title', {}),
        body: t('elements.notifications.kinds.subuserRemoved.body', { server }),
        details: [],
      };
    case 'backupCompleted':
      return {
        title: t('elements.notifications.kinds.backupCompleted.title', {}),
        body: t('elements.notifications.kinds.backupCompleted.body', { name: text(data.name), server }),
        details: [],
        serverPage: { path: '/backups', label: t('elements.notifications.viewBackups', {}) },
      };
    case 'backupFailed':
      return {
        title: t('elements.notifications.kinds.backupFailed.title', {}),
        body: t('elements.notifications.kinds.backupFailed.body', { name: text(data.name), server }),
        details: [],
        serverPage: { path: '/backups', label: t('elements.notifications.viewBackups', {}) },
      };
    // plugged.host: the partner program (docs/features/partners.md); about a Discord server, not a panel server
    case 'partnerApproved':
      return {
        title: t('elements.notifications.kinds.partnerApproved.title', {}),
        body: t('elements.notifications.kinds.partnerApproved.body', { guild, credits: credits(data.credits) }),
        details: [],
      };
    case 'partnerRejected':
      return {
        title: t('elements.notifications.kinds.partnerRejected.title', {}),
        body: t('elements.notifications.kinds.partnerRejected.body', { guild }),
        details: because(data.reason),
      };
    case 'partnerRevoked':
      return {
        title: t('elements.notifications.kinds.partnerRevoked.title', {}),
        body: t('elements.notifications.kinds.partnerRevoked.body', { guild }),
        details: because(data.reason),
      };
    case 'partnerPaid':
      return {
        title: t('elements.notifications.kinds.partnerPaid.title', {}),
        body: t('elements.notifications.kinds.partnerPaid.body', { guild, credits: credits(data.credits) }),
        details: [],
      };
    case 'partnerCheckFailed':
      return {
        title: t('elements.notifications.kinds.partnerCheckFailed.title', {}),
        body: t('elements.notifications.kinds.partnerCheckFailed.body', { guild, until: day(data.until) }),
        details: [],
      };
    case 'partnerSuspended':
      return {
        title: t('elements.notifications.kinds.partnerSuspended.title', {}),
        body: t('elements.notifications.kinds.partnerSuspended.body', { guild }),
        details: [],
      };
    case 'partnerPaused':
      return {
        title: t('elements.notifications.kinds.partnerPaused.title', {}),
        body: t('elements.notifications.kinds.partnerPaused.body', { guild }),
        details: because(data.reason),
      };
    case 'partnerResumed':
      return {
        title: t('elements.notifications.kinds.partnerResumed.title', {}),
        body: t('elements.notifications.kinds.partnerResumed.body', { guild }),
        details: [],
      };
    case 'partnerCreditsChanged':
      return {
        title: t('elements.notifications.kinds.partnerCreditsChanged.title', {}),
        body: t('elements.notifications.kinds.partnerCreditsChanged.body', { guild, credits: credits(data.credits) }),
        details: [],
      };
    case 'admin.partnerApplication':
      return {
        title: t('elements.notifications.kinds.admin.partnerApplication.title', { guild }),
        body: t('elements.notifications.kinds.admin.partnerApplication.body', { guild, user: text(data.user) }),
        details: [],
      };
    case 'admin.partnerSuspended':
      return {
        title: t('elements.notifications.kinds.admin.partnerSuspended.title', { guild }),
        body: t('elements.notifications.kinds.admin.partnerSuspended.body', { guild }),
        details: [],
      };
    default:
      // a kind this panel does not know yet (sent by a newer backend): its server's name, if any
      return { title: t('elements.notifications.unknown', {}), body: server || null, details: [] };
  }
}

interface Props {
  notification: Notification;
  onOpenServer: (notification: Notification) => void;
  onMarkRead: (notification: Notification) => void;
  onDelete: (notification: Notification) => void;
}

export default function NotificationRow({ notification, onOpenServer, onMarkRead, onDelete }: Props) {
  const translations = useTranslations();
  const { t } = translations;
  const { title, body, details, serverPage } = notificationText(translations, notification);
  const unread = notification.read === null;
  const deleteLabel = t('elements.notifications.delete', {});

  return (
    <div role='listitem' className={classNames('theme-notification', unread && 'theme-notification-unread')}>
      {unread && <span className='sr-only'>{t('elements.notifications.unreadLabel', {})}</span>}

      <div className='theme-notification-head'>
        <p className='theme-notification-title'>{title}</p>
        <Tooltip label={deleteLabel}>
          <CloseButton size='sm' aria-label={deleteLabel} onClick={() => onDelete(notification)} />
        </Tooltip>
      </div>

      <FormattedTimestamp timestamp={notification.created} className='theme-notification-date' />

      {body && <p className='theme-notification-body'>{body}</p>}
      {details.map((detail, index) => (
        <p key={index} className='theme-notification-body'>
          {detail}
        </p>
      ))}

      {(notification.server || unread) && (
        <div className='theme-notification-actions'>
          {notification.server && (
            <Link
              to={`/server/${notification.server.uuidShort}${serverPage?.path ?? ''}`}
              className='theme-notification-action'
              onClick={() => onOpenServer(notification)}
            >
              {serverPage?.label ?? t('elements.notifications.openServer', {})}
            </Link>
          )}
          {unread && (
            <button type='button' className='theme-notification-action' onClick={() => onMarkRead(notification)}>
              {t('elements.notifications.markRead', {})}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
