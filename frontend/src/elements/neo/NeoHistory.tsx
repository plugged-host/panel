// plugged.host: the earlier chats of this place (the Pelican panel's history view): open one to
// continue it, delete one with a second tap
import { faCommentMedical, faTrash } from '@fortawesome/free-solid-svg-icons';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import classNames from 'classnames';
import { useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import getNeoConversations from '@/api/me/neo/getNeoConversations.ts';
import Spinner from '@/elements/feedback/Spinner.tsx';
import NeoHead, { NeoIconButton } from '@/elements/neo/NeoHead.tsx';
import { formatTimestamp } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import type { NeoHistoryEntry } from '@/lib/schemas/user/neo.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { NeoChat, NeoPlace, newNeoConversation, removeNeoConversation, reopenNeoConversation } from '@/stores/neo.ts';

// how long the delete button waits for the second tap
const CONFIRM_MS = 4000;

function Entry({
  entry,
  place,
  current,
  working,
}: {
  entry: NeoHistoryEntry;
  place: NeoPlace;
  current: boolean;
  working: boolean;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [sure, setSure] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!sure) return;

    const timer = setTimeout(() => setSure(false), CONFIRM_MS);
    return () => clearTimeout(timer);
  }, [sure]);

  const remove = () => {
    if (!sure) {
      setSure(true);
      return;
    }

    setSure(false);
    setBusy(true);
    removeNeoConversation(place, entry.uuid)
      .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.user.neo.conversations(place.serverUuid) }))
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setBusy(false));
  };

  const count =
    entry.userMessages === 1
      ? t('elements.neo.history.oneMessage', {})
      : t('elements.neo.history.messages', { count: entry.userMessages });

  return (
    <div className={classNames('theme-neo-hist', current && 'theme-neo-hist-current')}>
      <button
        type='button'
        className='theme-neo-hist-open'
        disabled={working || busy}
        aria-current={current || undefined}
        onClick={() => void reopenNeoConversation(place, entry.uuid)}
      >
        <span className='theme-neo-hist-text'>
          <span className='theme-neo-hist-title'>{entry.title || t('elements.neo.history.untitled', {})}</span>
          <span className='theme-neo-hist-meta'>
            {formatTimestamp(entry.lastActivity)} · {count}
          </span>
        </span>
      </button>
      <NeoIconButton
        icon={faTrash}
        label={sure ? t('elements.neo.button.confirmDelete', {}) : t('elements.neo.button.deleteChat', {})}
        className={sure ? 'theme-neo-bad' : undefined}
        disabled={working || busy}
        onClick={remove}
      />
    </div>
  );
}

export default function NeoHistory({ place, chat }: { place: NeoPlace; chat: NeoChat }) {
  const { t } = useTranslations();
  const { addToast } = useToast();

  const conversations = useQuery({
    queryKey: queryKeys.user.neo.conversations(place.serverUuid),
    queryFn: () => getNeoConversations(place.serverUuid),
    // the list changes with every chat: read it each time the view opens
    staleTime: 0,
  });

  useEffect(() => {
    if (conversations.error) addToast(httpErrorToHuman(conversations.error), 'error');
  }, [conversations.error, addToast]);

  return (
    <>
      <NeoHead back sub={t('elements.neo.header.history', {})}>
        <NeoIconButton
          icon={faCommentMedical}
          label={t('elements.neo.button.newChat', {})}
          disabled={chat.working}
          onClick={() => void newNeoConversation(place)}
        />
      </NeoHead>
      <div className='theme-neo-settings'>
        {conversations.isLoading ? (
          <Spinner.Centered />
        ) : conversations.data && conversations.data.length > 0 ? (
          conversations.data.map((entry) => (
            <Entry
              key={entry.uuid}
              entry={entry}
              place={place}
              current={entry.uuid === chat.conversation?.uuid}
              working={chat.working}
            />
          ))
        ) : (
          <div className='theme-neo-empty'>
            <div>{t('elements.neo.history.empty', {})}</div>
          </div>
        )}
      </div>
    </>
  );
}
