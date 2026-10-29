// plugged.host: the chat of a server's owner and subusers (docs/features/server-chat.md), in neo's look:
// one's own messages as bubbles at the end, the others' as text beside their avatar, runs and days.
import { faArrowUp } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InfiniteData, useInfiniteQuery, useQueryClient } from '@tanstack/react-query';
import {
  FormEvent,
  KeyboardEvent,
  ReactNode,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getServerChat from '@/api/server/getServerChat.ts';
import sendServerChatMessage from '@/api/server/sendServerChatMessage.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Avatar from '@/elements/data-display/Avatar.tsx';
import ErrorState from '@/elements/feedback/ErrorState.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ProfileBadge from '@/elements/ProfileBadge.tsx'; // plugged.host
import { formatDateTime } from '@/lib/format/time.ts';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverChatMessageSchema } from '@/lib/schemas/server/chat.ts';
import { CHAT_RUN_GAP_MS, ChatRun, chatDayLabel, chatTimeLabel, groupChat, mergeChatPages } from '@/lib/serverChat.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useContainerAutoHeight } from '@/plugins/viewport/useContainerAutoHeight.ts';
import { useVisualViewportBottomInset } from '@/plugins/viewport/useVisualViewport.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useCurrentWindow } from '@/providers/CurrentWindowProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type ChatMessage = z.infer<typeof serverChatMessageSchema>;

const PER_PAGE = 50;
// the API's limit; the counter shows from COUNT_FROM on
const MAX_LENGTH = 2000;
const COUNT_FROM = 1800;
// how close to the bottom still counts as reading the newest message
const PINNED_PX = 48;
// how close to the top loads the page before
const OLDER_PX = 160;
// the field grows with its text up to this height, then scrolls
const MAX_INPUT_PX = 160;

/** The message the reader's view starts at, and how far below the view's top it is. */
interface Anchor {
  uuid: string;
  offset: number;
}

function ChatTime({ date, now, locale }: { date: Date; now: Date; locale: string }) {
  return (
    <Tooltip label={formatDateTime(date, false, false)}>
      <time className='theme-chat-time' dateTime={date.toISOString()}>
        {chatTimeLabel(date, now, locale)}
      </time>
    </Tooltip>
  );
}

function Run({
  run,
  now,
  locale,
  trailing,
}: {
  run: ChatRun<ChatMessage>;
  now: Date;
  locale: string;
  /** The message on its way, drawn at the end of the reader's own last run. */
  trailing?: ReactNode;
}) {
  const { t } = useTranslations();

  if (run.own) {
    return (
      <div className='theme-chat-run theme-chat-run-own'>
        <div className='theme-chat-meta'>
          <ChatTime date={run.created} now={now} locale={locale} />
        </div>
        {run.messages.map((message) => (
          <div key={message.uuid} className='theme-chat-bubble' data-message={message.uuid} dir='auto'>
            {message.message}
          </div>
        ))}
        {trailing}
      </div>
    );
  }

  const name = run.user?.username ?? t('pages.server.chat.deletedUser', {});

  return (
    <div className='theme-chat-run'>
      {run.user ? (
        <Avatar src={run.user.avatar} name={run.user.username} size={28} />
      ) : (
        <span className='theme-chat-avatar-none' />
      )}
      <div className='theme-chat-lines'>
        <div className='theme-chat-meta'>
          <span className='theme-chat-name' dir='auto'>
            {name}
          </span>
          <ProfileBadge badge={run.user?.badge} />
          <ChatTime date={run.created} now={now} locale={locale} />
        </div>
        {run.messages.map((message) => (
          <div key={message.uuid} className='theme-chat-text' data-message={message.uuid} dir='auto'>
            {message.message}
          </div>
        ))}
      </div>
    </div>
  );
}

export default function ServerChat() {
  const { t, language } = useTranslations();
  const server = useServerStore((state) => state.server);
  const { user } = useAuth();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = useServerCan('chat.write');
  const { getParent } = useCurrentWindow();
  const keyboardInset = useVisualViewportBottomInset();
  const counterId = useId();

  const [draft, setDraft] = useState('');
  const [pending, setPending] = useState<string | null>(null);
  const [unseen, setUnseen] = useState(false);
  const [now, setNow] = useState(() => new Date());

  const rootRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const threadRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const pinned = useRef(true);
  const anchor = useRef<Anchor | null>(null);
  const anchorFrame = useRef(0);
  const newest = useRef<string | null>(null);
  const loadingOlder = useRef(false);

  const queryKey = [...queryKeys.server(server.uuid).chat.all(), 'messages'];
  const { data, isLoading, error, refetch, isRefetching, fetchNextPage, hasNextPage, isFetchingNextPage } =
    useInfiniteQuery({
      queryKey,
      queryFn: ({ pageParam }) => getServerChat(server.uuid, pageParam, PER_PAGE),
      initialPageParam: 1,
      getNextPageParam: (lastPage) =>
        lastPage.page * lastPage.perPage < lastPage.total ? lastPage.page + 1 : undefined,
      refetchInterval: 5000,
    });

  const messages = useMemo(() => mergeChatPages(data?.pages.map((page) => page.data) ?? []), [data]);
  const days = useMemo(() => groupChat(messages, user?.uuid), [messages, user?.uuid]);
  const locale = intlLocale(language);

  // the card ends at the window's foot, above the keyboard on a phone (--theme-chat-height in app.css)
  useContainerAutoHeight({
    containerRef: rootRef,
    loading: false,
    getParent,
    layout: () => undefined,
    cssVariable: '--theme-chat-height',
    useVisualViewportInset: true,
    deps: [getParent],
  });

  // "now" and "5 minutes ago" move on, and Today becomes Yesterday at midnight
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 30_000);
    return () => clearInterval(timer);
  }, []);

  const rememberAnchor = () => {
    const body = bodyRef.current;
    if (!body) return;

    const top = body.getBoundingClientRect().top;
    const rows = body.querySelectorAll<HTMLElement>('[data-message]');
    // the first message whose bottom is below the view's top; rows are in reading order
    let low = 0;
    let high = rows.length - 1;
    let found = -1;
    while (low <= high) {
      const middle = (low + high) >> 1;
      if (rows[middle].getBoundingClientRect().bottom > top) {
        found = middle;
        high = middle - 1;
      } else {
        low = middle + 1;
      }
    }

    anchor.current =
      found < 0
        ? null
        : { uuid: rows[found].dataset.message ?? '', offset: rows[found].getBoundingClientRect().top - top };
  };

  // older messages above, the oldest page dropping one off its top, a wider window: the message the
  // reader was looking at stays where it was
  const restoreAnchor = () => {
    const body = bodyRef.current;
    const saved = anchor.current;
    if (!body || !saved) return;

    const row = body.querySelector<HTMLElement>(`[data-message="${saved.uuid}"]`);
    if (!row) return;

    const shift = row.getBoundingClientRect().top - body.getBoundingClientRect().top - saved.offset;
    if (Math.abs(shift) >= 1) body.scrollTop += shift;
  };

  const toBottom = () => {
    const body = bodyRef.current;
    if (body) body.scrollTop = body.scrollHeight;
  };

  const loadOlder = () => {
    if (loadingOlder.current || !hasNextPage) return;

    loadingOlder.current = true;
    void fetchNextPage().finally(() => {
      loadingOlder.current = false;
    });
  };

  // a change of the list: follow the newest while the reader is at the bottom, else keep their place
  // and say that something new arrived below
  const shape = `${messages[0]?.uuid}|${messages.at(-1)?.uuid}|${messages.length}|${pending ?? ''}`;
  useLayoutEffect(() => {
    const body = bodyRef.current;
    if (!body) return;

    const last = messages.at(-1)?.uuid ?? null;
    const before = newest.current;
    newest.current = last;

    if (pinned.current) {
      toBottom();
    } else {
      restoreAnchor();

      if (before && last !== before) {
        const index = messages.findIndex((message) => message.uuid === before);
        if (index >= 0 && messages.slice(index + 1).some((message) => message.user?.uuid !== user?.uuid)) {
          setUnseen(true);
        }
      }
    }

    // a first page that does not fill the view has nothing to scroll: fetch the one before right away
    if (messages.length > 0 && body.scrollHeight <= body.clientHeight) loadOlder();
  }, [shape]);

  // the thread or the view changing size (a wider window, the keyboard opening) keeps the same place
  useEffect(() => {
    const body = bodyRef.current;
    const thread = threadRef.current;
    if (!body || !thread) return;

    let frame = 0;
    const observer = new ResizeObserver(() => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (pinned.current) toBottom();
        else restoreAnchor();
      });
    });
    observer.observe(body);
    observer.observe(thread);

    return () => {
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  useEffect(() => () => cancelAnimationFrame(anchorFrame.current), []);

  const onScroll = () => {
    const body = bodyRef.current;
    if (!body) return;

    pinned.current = body.scrollHeight - body.scrollTop - body.clientHeight < PINNED_PX;
    if (pinned.current) setUnseen(false);
    if (body.scrollTop < OLDER_PX) loadOlder();

    if (!anchorFrame.current) {
      anchorFrame.current = requestAnimationFrame(() => {
        anchorFrame.current = 0;
        rememberAnchor();
      });
    }
  };

  const jumpToNewest = () => {
    pinned.current = true;
    setUnseen(false);
    toBottom();
  };

  // the field follows its text, also when the text changes from outside (sent, given back)
  useLayoutEffect(() => {
    const input = inputRef.current;
    if (!input) return;

    input.style.height = 'auto';
    input.style.height = `${Math.min(input.scrollHeight, MAX_INPUT_PX)}px`;
  }, [draft]);

  const send = async (event?: FormEvent) => {
    event?.preventDefault();

    const text = draft.trim();
    if (!text || pending !== null) return;

    pinned.current = true;
    setUnseen(false);
    setPending(text);
    setDraft('');

    try {
      const message = await sendServerChatMessage(server.uuid, text);

      // the newest page has it at once; the refetch puts every page right
      queryClient.setQueryData<InfiniteData<Pagination<ChatMessage>, number>>(queryKey, (old) =>
        old && old.pages.length > 0
          ? {
              ...old,
              pages: [
                { ...old.pages[0], total: old.pages[0].total + 1, data: [...old.pages[0].data, message] },
                ...old.pages.slice(1),
              ],
            }
          : old,
      );
      void queryClient.invalidateQueries({ queryKey });
    } catch (err) {
      // the text comes back, before whatever was typed meanwhile
      setDraft((current) => (current ? `${text}\n${current}` : text));
      addToast(httpErrorToHuman(err), 'error');
    } finally {
      setPending(null);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter sends on a keyboard; on a touch screen it is a new line and the button sends (as in neo)
    if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return;
    if (window.matchMedia('(pointer: coarse)').matches) return;

    event.preventDefault();
    void send();
  };

  const pendingBubble =
    pending !== null ? (
      <div className='theme-chat-bubble theme-chat-pending' dir='auto' aria-busy>
        {pending}
      </div>
    ) : null;
  // on its way, it joins the reader's own last run while that is recent, as it will once stored
  const lastRun = days.at(-1)?.runs.at(-1);
  const lastCreated = lastRun?.messages.at(-1)?.created.getTime() ?? 0;
  const pendingJoins =
    pendingBubble !== null && lastRun?.own === true && now.getTime() - lastCreated <= CHAT_RUN_GAP_MS;
  const showCount = draft.length >= COUNT_FROM;

  let thread: ReactNode;
  if (isLoading) {
    thread = <Spinner.Centered className='my-auto' />;
  } else if (error && !data) {
    // a failed poll after the first answer keeps the messages on screen; this is a chat that never loaded
    thread = (
      <div className='my-auto'>
        <ErrorState error={httpErrorToHuman(error)} onRetry={() => void refetch()} retrying={isRefetching} />
      </div>
    );
  } else if (messages.length === 0 && pending === null) {
    thread = (
      <p className='theme-chat-empty'>
        {canWrite ? t('pages.server.chat.empty', {}) : t('pages.server.chat.emptyReadOnly', {})}
      </p>
    );
  } else {
    thread = (
      <>
        {hasNextPage && (
          <div className='theme-chat-older'>
            <Button variant='subtle' color='gray' size='compact-sm' loading={isFetchingNextPage} onClick={loadOlder}>
              {t('pages.server.chat.loadOlder', {})}
            </Button>
          </div>
        )}
        {days.map((day) => (
          <div key={day.key} className='theme-chat-day'>
            <div className='theme-chat-day-label'>{chatDayLabel(day.date, now, locale)}</div>
            {day.runs.map((run) => (
              <Run
                key={run.key}
                run={run}
                now={now}
                locale={locale}
                trailing={pendingJoins && run === lastRun ? pendingBubble : undefined}
              />
            ))}
          </div>
        ))}
        {pendingBubble && !pendingJoins && (
          <div className='theme-chat-run theme-chat-run-own'>
            <div className='theme-chat-meta'>
              <span className='theme-chat-time'>{chatTimeLabel(now, now, locale)}</span>
            </div>
            {pendingBubble}
          </div>
        )}
      </>
    );
  }

  return (
    <ServerContentContainer title={t('pages.server.chat.title', {})}>
      <div ref={rootRef} className='theme-chat' data-keyboard={keyboardInset > 0 ? '' : undefined}>
        <div className='theme-chat-view'>
          <div
            ref={bodyRef}
            className='theme-chat-body'
            role='log'
            aria-label={t('pages.server.chat.title', {})}
            // a scrolled region a keyboard can reach and scroll
            tabIndex={0}
            onScroll={onScroll}
          >
            <div ref={threadRef} className='theme-chat-thread'>
              {thread}
            </div>
          </div>

          {unseen && (
            <button type='button' className='theme-chat-jump' onClick={jumpToNewest}>
              {t('pages.server.chat.newMessages', {})}
            </button>
          )}
        </div>

        <div className='theme-chat-foot'>
          {canWrite ? (
            <form className='theme-neo-compose' onSubmit={(event) => void send(event)}>
              <textarea
                ref={inputRef}
                className='theme-neo-input'
                rows={1}
                maxLength={MAX_LENGTH}
                value={draft}
                placeholder={t('pages.server.chat.placeholder', {})}
                aria-label={t('pages.server.chat.placeholder', {})}
                aria-describedby={showCount ? counterId : undefined}
                onChange={(event) => setDraft(event.currentTarget.value)}
                onKeyDown={onKeyDown}
              />
              {showCount && (
                <span
                  id={counterId}
                  className='theme-chat-count'
                  data-full={draft.length >= MAX_LENGTH ? '' : undefined}
                >
                  {draft.length}/{MAX_LENGTH}
                </span>
              )}
              <Tooltip label={t('common.button.send', {})}>
                <button
                  type='submit'
                  className='theme-neo-send'
                  aria-label={t('common.button.send', {})}
                  disabled={!draft.trim() || pending !== null}
                >
                  <FontAwesomeIcon icon={faArrowUp} />
                </button>
              </Tooltip>
            </form>
          ) : (
            <p className='theme-chat-readonly'>{t('pages.server.chat.readOnly', {})}</p>
          )}
        </div>
      </div>
    </ServerContentContainer>
  );
}
