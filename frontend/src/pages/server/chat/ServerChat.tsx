import { faPaperPlane } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Center, Loader, Paper, ScrollArea, Stack, Text, TextInput } from '@mantine/core';
import { useInfiniteQuery, useQueryClient } from '@tanstack/react-query';
import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import getServerChat from '@/api/server/getServerChat.ts';
import sendServerChatMessage from '@/api/server/sendServerChatMessage.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Avatar from '@/elements/data-display/Avatar.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

const PER_PAGE = 50;

export default function ServerChat() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const { user } = useAuth();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = useServerCan('chat.write');

  const [message, setMessage] = useState('');
  const [sending, setSending] = useState(false);
  const viewport = useRef<HTMLDivElement>(null);
  const atBottom = useRef(true);

  const queryKey = [...queryKeys.server(server.uuid).chat.all(), 'messages'];
  const { data, isLoading, error, fetchNextPage, hasNextPage, isFetchingNextPage } = useInfiniteQuery({
    queryKey,
    queryFn: ({ pageParam }) => getServerChat(server.uuid, pageParam, PER_PAGE),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => (lastPage.page * lastPage.perPage < lastPage.total ? lastPage.page + 1 : undefined),
    refetchInterval: 5000,
  });

  // Pages arrive newest first; each page is already oldest to newest. Oldest page first reads
  // like a chat.
  const messages = useMemo(() => (data ? [...data.pages].reverse().flatMap((page) => page.data) : []), [data]);

  const scrollToBottom = (smooth = false) => {
    const element = viewport.current;
    if (!element) return;

    element.scrollTo({ top: element.scrollHeight, behavior: smooth ? 'smooth' : 'auto' });
  };

  useEffect(() => {
    if (atBottom.current) scrollToBottom();
  }, [messages.length]);

  const onScroll = () => {
    const element = viewport.current;
    if (!element) return;

    atBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80;
  };

  const doSend = async (event: FormEvent) => {
    event.preventDefault();

    const text = message.trim();
    if (!text || sending) return;

    setSending(true);
    try {
      await sendServerChatMessage(server.uuid, text);
      setMessage('');
      atBottom.current = true;
      await queryClient.invalidateQueries({ queryKey });
      scrollToBottom(true);
    } catch (err) {
      addToast(httpErrorToHuman(err), 'error');
    } finally {
      setSending(false);
    }
  };

  return (
    <ServerContentContainer title={t('pages.server.chat.title', {})}>
      <Paper className='flex h-[calc(100dvh-16rem)] min-h-96 flex-col overflow-hidden'>
        {isLoading ? (
          <Center className='flex-1'>
            <Loader />
          </Center>
        ) : error ? (
          <Center className='flex-1'>
            <Text c='red'>{httpErrorToHuman(error)}</Text>
          </Center>
        ) : messages.length === 0 ? (
          <EmptyState title={t('pages.server.chat.empty', {})} />
        ) : (
          <ScrollArea className='flex-1' viewportRef={viewport} onScrollPositionChange={onScroll}>
            <Stack gap='sm' p='md'>
              {hasNextPage && (
                <Center>
                  <Button variant='subtle' size='xs' loading={isFetchingNextPage} onClick={() => void fetchNextPage()}>
                    {t('pages.server.chat.loadOlder', {})}
                  </Button>
                </Center>
              )}
              {messages.map((entry) => {
                const own = entry.user?.uuid === user?.uuid;

                return (
                  <div key={entry.uuid} className={`flex gap-2 ${own ? 'flex-row-reverse' : 'flex-row'}`}>
                    <Avatar src={entry.user?.avatar} name={entry.user?.username} size='sm' />
                    <div className={`max-w-[80%] ${own ? 'items-end' : 'items-start'} flex flex-col`}>
                      <div className='mb-0.5 flex items-baseline gap-2'>
                        <Text size='xs' fw={600}>
                          {entry.user?.username ?? t('pages.server.chat.deletedUser', {})}
                        </Text>
                        <Text size='xs' c='dimmed'>
                          {formatDateTime(entry.created)}
                        </Text>
                      </div>
                      <Paper
                        p='xs'
                        radius='md'
                        bg={own ? 'var(--mantine-primary-color-light)' : 'var(--mantine-color-default-hover)'}
                      >
                        <Text size='sm' className='whitespace-pre-wrap break-words'>
                          {entry.message}
                        </Text>
                      </Paper>
                    </div>
                  </div>
                );
              })}
            </Stack>
          </ScrollArea>
        )}

        {canWrite && (
          <form onSubmit={doSend} className='flex gap-2 border-t border-(--mantine-color-default-border) p-3'>
            <TextInput
              className='flex-1'
              placeholder={t('pages.server.chat.placeholder', {})}
              value={message}
              maxLength={2000}
              onChange={(event) => setMessage(event.currentTarget.value)}
            />
            <Button type='submit' loading={sending} disabled={!message.trim()}>
              <FontAwesomeIcon icon={faPaperPlane} />
            </Button>
          </form>
        )}
      </Paper>
    </ServerContentContainer>
  );
}
