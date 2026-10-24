// plugged.host: neo's chat (docs/features/neo.md), as the Pelican panel has it: a launcher in the
// window's corner and the chat as a panel above it (full screen on phones), the chat of the page
// the person is on. The loop lives in stores/neo.ts, so a turn carries on while the chat is
// minimized or the person moves between pages.
import { faClockRotateLeft, faCommentMedical, faGear } from '@fortawesome/free-solid-svg-icons';
import { CSSProperties, PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import NeoComposer from '@/elements/neo/NeoComposer.tsx';
import { NeoConsent, NeoLoading, NeoNote, NeoOff } from '@/elements/neo/NeoGates.tsx';
import NeoHead, { NeoIconButton } from '@/elements/neo/NeoHead.tsx';
import NeoHistory from '@/elements/neo/NeoHistory.tsx';
import NeoLauncher from '@/elements/neo/NeoLauncher.tsx';
import NeoSettings from '@/elements/neo/NeoSettings.tsx';
import NeoThread from '@/elements/neo/NeoThread.tsx';
import { useNeoName, useNeoPlace } from '@/plugins/useNeo.ts';
import { usePageBreakpoint } from '@/plugins/viewport/usePageBreakpoint.ts';
import { getTranslations, useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import {
  getNeoStore,
  loadNeo,
  NeoChat as NeoChatState,
  NeoPlace,
  newNeoConversation,
  openNeo,
  useNeoChat,
  useNeoStore,
} from '@/stores/neo.ts';

// the panel's size as the person dragged it (a per-browser convenience)
const SIZE_KEY = 'neo-chat:size';
const MIN_WIDTH = 300;
const MIN_HEIGHT = 320;

interface Size {
  w: number;
  h: number;
}

function readSize(): Size | null {
  try {
    const size = JSON.parse(window.localStorage.getItem(SIZE_KEY) ?? 'null');
    return size && typeof size.w === 'number' && typeof size.h === 'number' ? size : null;
  } catch {
    return null;
  }
}

function storeSize(size: Size | null) {
  try {
    if (size) window.localStorage.setItem(SIZE_KEY, JSON.stringify(size));
    else window.localStorage.removeItem(SIZE_KEY);
  } catch {
    // the size only lasts until the page is reloaded
  }
}

/** The chat view: its head with history, new chat and settings, then the notice or the conversation. */
function ChatView({ place, chat }: { place: NeoPlace; chat: NeoChatState }) {
  const { t } = useTranslations();
  const name = useNeoName();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const setView = useNeoStore((state) => state.setView);
  const info = chat.info;

  if (!info) return null;

  return (
    <>
      <NeoHead sub={place.serverName ?? t('elements.neo.header.account', { app: appName })}>
        <NeoIconButton
          icon={faClockRotateLeft}
          label={t('elements.neo.button.history', {})}
          onClick={() => setView('history')}
        />
        <NeoIconButton
          icon={faCommentMedical}
          label={t('elements.neo.button.newChat', {})}
          disabled={chat.working}
          onClick={() => void newNeoConversation(place)}
        />
        <NeoIconButton
          icon={faGear}
          label={t('elements.neo.button.settings', { name })}
          onClick={() => setView('settings')}
        />
      </NeoHead>
      {info.blocked === 'consent' ? (
        <NeoConsent info={info} />
      ) : (
        <>
          <NeoThread place={place} chat={chat} />
          <NeoComposer place={place} info={info} chat={chat} />
        </>
      )}
    </>
  );
}

/** What the panel shows: a view of its own while neo cannot chat here, else the chosen view. */
function Content({ place }: { place: NeoPlace }) {
  const { t } = useTranslations();
  const name = useNeoName();
  const view = useNeoStore((state) => state.view);
  const chat = useNeoChat(place.key, (state) => state);
  const info = chat.info;

  if (!info) {
    return (
      <>
        <NeoHead sub={place.serverName ?? name} />
        {chat.failed && !chat.loading ? (
          <NeoNote title={t('elements.neo.blocked.failed', { name })} body={chat.failed} retry={place} />
        ) : (
          <NeoLoading />
        )}
      </>
    );
  }

  switch (info.blocked) {
    case 'turned_off':
      return (
        <>
          <NeoHead sub={t('elements.neo.launcher.off', { name })} />
          <NeoOff />
        </>
      );
    case 'disabled':
      return (
        <>
          <NeoHead sub={place.serverName ?? name} />
          <NeoNote title={t('elements.neo.blocked.disabled', { name })} />
        </>
      );
    case 'no_access':
      return (
        <>
          <NeoHead sub={place.serverName ?? name} />
          <NeoNote title={t('elements.neo.blocked.noAccess', { name })} />
        </>
      );
    case 'not_yours':
      return (
        <>
          <NeoHead sub={place.serverName ?? name} />
          <NeoNote title={t('elements.neo.notice.notYours', {})} retry={place} />
        </>
      );
  }

  if (view === 'settings') return <NeoSettings info={info} chat={chat} />;
  if (view === 'history') return <NeoHistory place={place} chat={chat} />;

  return <ChatView place={place} chat={chat} />;
}

export default function NeoChat() {
  const { t } = useTranslations();
  const name = useNeoName();
  const navigate = useNavigate();
  const place = useNeoPlace();
  const open = useNeoStore((state) => state.open);
  const redirect = useNeoStore((state) => state.redirect);
  const wide = usePageBreakpoint('sm');
  const [size, setSize] = useState<Size | null>(readSize);
  const panelRef = useRef<HTMLElement>(null);
  const launcherRef = useRef<HTMLButtonElement>(null);
  const wasOpen = useRef(open);

  // the chat follows the page: a server's own chat on its pages, the account's elsewhere
  useEffect(() => {
    const store = getNeoStore();
    if (store.place?.key !== place?.key) {
      useNeoStore.setState({ view: 'chat' });
    }
    store.setPlace(place);
  }, [place]);

  // opening the chat (or arriving on a page with it open) loads it and resumes a running turn
  useEffect(() => {
    if (open && place) void loadNeo(place);
  }, [open, place]);

  // a start gate a tool hit (the queue, the ad room): the person goes there
  useEffect(() => {
    if (!redirect) return;

    getNeoStore().clearRedirect();
    if (redirect.startsWith('/') && !redirect.startsWith('//')) {
      navigate(redirect);
    } else if (/^https?:\/\//i.test(redirect)) {
      window.location.assign(redirect);
    }
  }, [redirect, navigate]);

  // "Ask neo to fix it" on the log check: `neo:ask` with `{ log }` (a mclo.gs id or link) opens the
  // server's chat and starts a new chat about that log, which neo reads first
  useEffect(() => {
    const onAsk = (event: Event) => {
      const detail = (event as CustomEvent<{ log?: unknown } | null>).detail;
      const log = typeof detail?.log === 'string' ? detail.log.trim() : '';
      if (!log || !getNeoStore().place?.serverUuid) return;

      const url = /^https?:\/\//i.test(log) ? log : `https://mclo.gs/${encodeURIComponent(log)}`;
      openNeo({
        text: getTranslations().t('elements.neo.askLog', { url }),
        images: [],
        seed: { tool: 'read_mclogs', arguments: { log } },
        fresh: true,
      });
    };

    window.addEventListener('neo:ask', onAsk);
    return () => window.removeEventListener('neo:ask', onAsk);
  }, []);

  // full screen on phones: the page behind it stays put
  useEffect(() => {
    const lock = open && !wide && !!place;
    document.documentElement.classList.toggle('theme-neo-lock', lock);

    return () => document.documentElement.classList.remove('theme-neo-lock');
  }, [open, wide, place]);

  // focus goes into the chat when it opens and back to the launcher when it is minimized
  useEffect(() => {
    if (open && !wasOpen.current) {
      panelRef.current?.focus({ preventScroll: true });
    } else if (!open && wasOpen.current) {
      const active = document.activeElement;
      if (!active || active === document.body) launcherRef.current?.focus({ preventScroll: true });
    }
    wasOpen.current = open;
  }, [open]);

  const startResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    const panel = panelRef.current;
    if (!panel || event.button !== 0) return;
    event.preventDefault();

    const rect = panel.getBoundingClientRect();
    const start = { x: event.clientX, y: event.clientY, w: rect.width, h: rect.height };
    let next: Size | null = null;

    const move = (moved: PointerEvent) => {
      next = {
        w: Math.round(Math.min(window.innerWidth - 32, Math.max(MIN_WIDTH, start.w + start.x - moved.clientX))),
        h: Math.round(Math.min(window.innerHeight - 96, Math.max(MIN_HEIGHT, start.h + start.y - moved.clientY))),
      };
      setSize(next);
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', up);
      document.documentElement.classList.remove('theme-neo-resizing');
      if (next) storeSize(next);
    };

    document.documentElement.classList.add('theme-neo-resizing');
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', up);
  };

  const resetSize = () => {
    setSize(null);
    storeSize(null);
  };

  if (!place) return null;

  const style = size
    ? ({ '--theme-neo-w': `${size.w}px`, '--theme-neo-h': `${size.h}px` } as CSSProperties)
    : undefined;

  return (
    <div className='theme-neo' style={style}>
      {open ? (
        <section ref={panelRef} className='theme-neo-panel' role='dialog' aria-label={name} tabIndex={-1}>
          {/* a pointer-only resize corner: the panel works at any size, so it is hidden from assistive tech */}
          <div
            className='theme-neo-grip'
            title={t('elements.neo.button.resize', {})}
            aria-hidden
            onPointerDown={startResize}
            onDoubleClick={resetSize}
          />
          <Content place={place} />
        </section>
      ) : (
        <NeoLauncher ref={launcherRef} place={place} />
      )}
    </div>
  );
}
