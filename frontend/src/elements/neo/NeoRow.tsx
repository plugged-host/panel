// plugged.host: one row of neo's chat, as the Pelican panel shows it: the person's message as a
// bubble, an answer as markdown (its thinking folded away), a tool call as a compact tile, a call
// waiting for approval as a card, a notice (a limit, an outage) in words
import {
  faBan,
  faBolt,
  faCheck,
  faCircleExclamation,
  faCircleInfo,
  faClock,
  faHand,
  faHourglass,
  faLink,
  faMinus,
  faSpinner,
  faTriangleExclamation,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import Button from '@/elements/buttons/Button.tsx';
import NeoImage from '@/elements/neo/NeoImage.tsx';
import NeoMarkdown from '@/elements/neo/NeoMarkdown.tsx';
import type { NeoConversation, NeoMessage } from '@/lib/schemas/user/neo.ts';
import { useNeoText } from '@/plugins/useNeo.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { approveNeoCall, denyNeoCall, NeoPlace, retryNeoTurn } from '@/stores/neo.ts';

type ToolStatus = NonNullable<NeoMessage['status']>;

function StatusIcon({ status }: { status: ToolStatus | null }) {
  switch (status) {
    case 'executing':
      return <FontAwesomeIcon icon={faSpinner} spin className='theme-neo-dim' />;
    case 'done':
      return <FontAwesomeIcon icon={faCheck} className='theme-neo-ok' />;
    case 'failed':
      return <FontAwesomeIcon icon={faCircleExclamation} className='theme-neo-bad' />;
    case 'denied':
      return <FontAwesomeIcon icon={faBan} className='theme-neo-bad' />;
    case 'skipped':
      return <FontAwesomeIcon icon={faMinus} className='theme-neo-dim' />;
    default:
      return <FontAwesomeIcon icon={faClock} className='theme-neo-dim' />;
  }
}

function useStatusWord() {
  const { t } = useTranslations();

  return (status: ToolStatus | null): string => {
    switch (status) {
      case 'pending_approval':
        return t('elements.neo.status.pendingApproval', {});
      case 'approved':
        return t('elements.neo.status.approved', {});
      case 'executing':
        return t('elements.neo.status.executing', {});
      case 'done':
        return t('elements.neo.status.done', {});
      case 'failed':
        return t('elements.neo.status.failed', {});
      case 'denied':
        return t('elements.neo.status.denied', {});
      case 'skipped':
        return t('elements.neo.status.skipped', {});
      default:
        return t('elements.neo.status.queued', {});
    }
  };
}

function Images({ uuids, answer = false }: { uuids: string[]; answer?: boolean }) {
  if (uuids.length === 0) return null;

  return (
    <div className={answer ? 'theme-neo-msg-images theme-neo-msg-images-ai' : 'theme-neo-msg-images'}>
      {uuids.map((uuid) => (
        <NeoImage key={uuid} uuid={uuid} />
      ))}
    </div>
  );
}

function Approval({ message, place, working }: { message: NeoMessage; place: NeoPlace; working: boolean }) {
  const { t } = useTranslations();
  const statusWord = useStatusWord();

  return (
    <div className='theme-neo-card'>
      <div className='theme-neo-tile' title={statusWord('pending_approval')}>
        <FontAwesomeIcon icon={faHand} className='theme-neo-warn' />
        <span className='sr-only'>{statusWord('pending_approval')}</span>
        <span className='theme-neo-tile-label'>{message.summary ?? message.tool}</span>
      </div>
      {message.risk === 'destructive' && <div className='theme-neo-risk'>{t('elements.neo.risk.destructive', {})}</div>}
      {message.risk === 'critical' && <div className='theme-neo-risk'>{t('elements.neo.risk.critical', {})}</div>}
      {message.preview && (
        <details className='theme-neo-preview'>
          <summary>{t('elements.neo.message.showChange', {})}</summary>
          <pre>{message.preview}</pre>
        </details>
      )}
      <div className='theme-neo-row'>
        <Button size='sm' variant='default' disabled={working} onClick={() => void approveNeoCall(place, message.uuid)}>
          {t('elements.neo.button.approve', {})}
        </Button>
        <Button
          size='sm'
          variant='subtle'
          color='gray'
          disabled={working}
          onClick={() => void denyNeoCall(place, message.uuid)}
        >
          {t('elements.neo.button.deny', {})}
        </Button>
      </div>
    </div>
  );
}

function Tool({ message }: { message: NeoMessage }) {
  const { t } = useTranslations();
  const statusWord = useStatusWord();
  const auto = message.approval === 'auto';
  const status = `${statusWord(message.status)}${auto ? ` · ${t('elements.neo.message.auto', {})}` : ''}`;

  return (
    <>
      <div className='theme-neo-tile' title={status}>
        <StatusIcon status={message.status} />
        <span className='sr-only'>{status}</span>
        <span className='theme-neo-tile-label'>{message.display ?? message.summary ?? message.tool}</span>
        {auto && <FontAwesomeIcon icon={faBolt} className='theme-neo-dim' />}
      </div>
      {message.status === 'failed' && message.content && (
        <details className='theme-neo-preview theme-neo-output'>
          <summary>{t('elements.neo.message.showOutput', {})}</summary>
          <pre>{message.content}</pre>
        </details>
      )}
    </>
  );
}

function noticeIcon(code: string | null) {
  if (code?.startsWith('limited_')) return faHourglass;
  if (code === 'unlinked') return faLink;
  if (code === 'refused') return faCircleInfo;

  return faTriangleExclamation;
}

function Notice({
  message,
  place,
  retry,
}: {
  message: NeoMessage;
  place: NeoPlace;
  /** The notice ends the chat and the turn can be tried again. */
  retry: boolean;
}) {
  const { t } = useTranslations();
  const text = useNeoText();

  return (
    <div className='theme-neo-notice' role='status'>
      <FontAwesomeIcon icon={noticeIcon(message.code)} />
      <div className='theme-neo-notice-body'>
        <span>{text.notice(message.code, message.params)}</span>
        {retry && (
          <Button size='sm' variant='default' onClick={() => void retryNeoTurn(place)}>
            {t('elements.neo.button.retry', {})}
          </Button>
        )}
      </div>
    </div>
  );
}

export default function NeoRow({
  message,
  conversation,
  place,
  last,
  working,
}: {
  message: NeoMessage;
  conversation: NeoConversation;
  place: NeoPlace;
  last: boolean;
  working: boolean;
}) {
  const { t } = useTranslations();

  switch (message.role) {
    case 'user':
      return (
        <>
          <Images uuids={message.images} />
          {message.content && <div className='theme-neo-msg-user'>{message.content}</div>}
        </>
      );
    case 'assistant':
      return (
        <>
          {message.thinking && (
            <details className='theme-neo-preview theme-neo-thought'>
              <summary>{t('elements.neo.message.thought', {})}</summary>
              <pre>{message.thinking}</pre>
            </details>
          )}
          {message.content && <NeoMarkdown text={message.content} />}
          <Images uuids={message.images} answer />
          {message.model && (message.content || message.images.length > 0) && (
            <div className='theme-neo-model-label'>{message.model}</div>
          )}
        </>
      );
    case 'tool':
      return message.status === 'pending_approval' ? (
        <Approval message={message} place={place} working={working} />
      ) : (
        <Tool message={message} />
      );
    default:
      return (
        <Notice
          message={message}
          place={place}
          retry={
            last &&
            !working &&
            message.code !== 'refused' &&
            ['error', 'limited', 'unlinked'].includes(conversation.status)
          }
        />
      );
  }
}
