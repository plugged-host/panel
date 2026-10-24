// plugged.host: the conversation of neo's chat: the rows, the answer while it is written, and the
// suggestions of an empty chat (the Pelican panel's ".neo-body"). It follows the newest row while
// it is read from the bottom; scrolled up, it stays put.
import { useEffect, useRef } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import NeoImage from '@/elements/neo/NeoImage.tsx';
import NeoMarkdown from '@/elements/neo/NeoMarkdown.tsx';
import NeoRow from '@/elements/neo/NeoRow.tsx';
import { useNeoName } from '@/plugins/useNeo.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { approveNeoCall, NeoChat, NeoPlace, sendNeo } from '@/stores/neo.ts';

// how close to the bottom still counts as reading the newest row
const PINNED_PX = 48;

function Empty({ place, chat }: { place: NeoPlace; chat: NeoChat }) {
  const { t } = useTranslations();
  const createEnabled = useGlobalStore((state) => state.settings.games.createEnabled);
  const tokensEnabled = useGlobalStore((state) => state.settings.economy.tokensEnabled);

  const suggestions = place.serverUuid
    ? [
        {
          label: t('elements.neo.suggestions.crash.label', {}),
          prompt: t('elements.neo.suggestions.crash.prompt', {}),
        },
        { label: t('elements.neo.suggestions.lag.label', {}), prompt: t('elements.neo.suggestions.lag.prompt', {}) },
        {
          label: t('elements.neo.suggestions.plugin.label', {}),
          prompt: t('elements.neo.suggestions.plugin.prompt', {}),
        },
        {
          label: t('elements.neo.suggestions.backup.label', {}),
          prompt: t('elements.neo.suggestions.backup.prompt', {}),
        },
      ]
    : [
        ...(createEnabled
          ? [
              {
                label: t('elements.neo.suggestions.create.label', {}),
                prompt: t('elements.neo.suggestions.create.prompt', {}),
              },
            ]
          : []),
        {
          label: t('elements.neo.suggestions.allowance.label', {}),
          prompt: t('elements.neo.suggestions.allowance.prompt', {}),
        },
        ...(tokensEnabled
          ? [
              {
                label: t('elements.neo.suggestions.tokens.label', {}),
                prompt: t('elements.neo.suggestions.tokens.prompt', {}),
              },
            ]
          : []),
      ];

  return (
    <div className='theme-neo-empty'>
      <div>
        <div className='theme-neo-greet'>
          {place.serverName
            ? t('elements.neo.empty.serverTitle', { server: place.serverName })
            : t('elements.neo.empty.accountTitle', {})}
        </div>
        <div>{place.serverUuid ? t('elements.neo.empty.serverBody', {}) : t('elements.neo.empty.accountBody', {})}</div>
      </div>
      <div className='theme-neo-chips'>
        {suggestions.map((suggestion) => (
          <button
            key={suggestion.label}
            type='button'
            className='theme-neo-chip'
            disabled={chat.working}
            // like a typed message: the images waiting in the composer go with it
            onClick={() => void sendNeo(place, { text: suggestion.prompt, images: chat.images }, true)}
          >
            {suggestion.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export default function NeoThread({ place, chat }: { place: NeoPlace; chat: NeoChat }) {
  const { t } = useTranslations();
  const name = useNeoName();
  const bodyRef = useRef<HTMLDivElement>(null);
  const threadRef = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);

  const conversation = chat.conversation;
  const messages = conversation?.messages ?? [];
  const waiting = messages.filter((message) => message.role === 'tool' && message.status === 'pending_approval');

  // whatever grows the thread (a row, a streamed word, an image that loaded) keeps the newest in view
  useEffect(() => {
    const body = bodyRef.current;
    const thread = threadRef.current;
    if (!body || !thread) return;

    let frame = 0;
    const stick = () => {
      if (!pinned.current || frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        body.scrollTop = body.scrollHeight;
      });
    };

    const observer = new ResizeObserver(stick);
    observer.observe(thread);
    stick();

    return () => {
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  // a new request is read from the bottom again
  useEffect(() => {
    if (chat.working) pinned.current = true;
  }, [chat.working]);

  const onScroll = () => {
    const body = bodyRef.current;
    if (!body) return;

    pinned.current = body.scrollHeight - body.scrollTop - body.clientHeight < PINNED_PX;
  };

  return (
    <div className='theme-neo-body' ref={bodyRef} onScroll={onScroll}>
      <div className='theme-neo-thread' ref={threadRef}>
        {messages.length === 0 && !chat.sending && <Empty place={place} chat={chat} />}

        {conversation &&
          messages.map((message, index) => (
            <NeoRow
              key={message.uuid}
              message={message}
              conversation={conversation}
              place={place}
              last={index === messages.length - 1}
              working={chat.working}
            />
          ))}

        {/* the message on its way, until the conversation has it */}
        {chat.sending && (
          <>
            {chat.sending.images.length > 0 && (
              <div className='theme-neo-msg-images'>
                {chat.sending.images.map((src) => (
                  <NeoImage key={src} src={src} />
                ))}
              </div>
            )}
            {chat.sending.text.trim() && <div className='theme-neo-msg-user'>{chat.sending.text.trim()}</div>}
          </>
        )}

        {waiting.length > 1 && (
          <Button size='sm' variant='default' disabled={chat.working} onClick={() => void approveNeoCall(place, null)}>
            {t('elements.neo.button.approveAll', { count: waiting.length })}
          </Button>
        )}

        {/* the answer while the model writes it; the stored message takes its place */}
        {chat.liveThinking && <div className='theme-neo-live-think'>{chat.liveThinking}</div>}
        {chat.liveText && <NeoMarkdown text={chat.liveText} />}

        {chat.working && (
          <div className='theme-neo-typing' role='status' aria-label={t('elements.neo.message.working', { name })}>
            <i />
            <i />
            <i />
          </div>
        )}
      </div>
    </div>
  );
}
