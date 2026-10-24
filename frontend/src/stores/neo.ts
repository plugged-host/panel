// plugged.host: neo's chat (docs/features/neo.md): whether it is open, each place's chat and the
// loop that drives it. The loop lives here, outside the pages, so a running turn carries on while
// the person navigates or closes the chat; elements/neo/ only shows this state.
import { create } from 'zustand';
import { getHttpStatus, httpErrorToHuman } from '@/api/axios.ts';
import approveNeo from '@/api/me/neo/approveNeo.ts';
import cancelNeo from '@/api/me/neo/cancelNeo.ts';
import deleteNeoChats from '@/api/me/neo/deleteNeoChats.ts';
import deleteNeoConversation from '@/api/me/neo/deleteNeoConversation.ts';
import denyNeo from '@/api/me/neo/denyNeo.ts';
import getNeo from '@/api/me/neo/getNeo.ts';
import getNeoConversation from '@/api/me/neo/getNeoConversation.ts';
import newNeoChat from '@/api/me/neo/newNeoChat.ts';
import openNeoConversation from '@/api/me/neo/openNeoConversation.ts';
import retryNeo from '@/api/me/neo/retryNeo.ts';
import sendNeoMessage from '@/api/me/neo/sendNeoMessage.ts';
import stepNeo from '@/api/me/neo/stepNeo.ts';
import updateNeoPreferences from '@/api/me/neo/updateNeoPreferences.ts';
import type {
  NeoConversation,
  NeoInfo,
  NeoPreferencesUpdate,
  NeoSeed,
  NeoStepResponse,
  NeoStepResult,
  NeoStreamResult,
} from '@/lib/schemas/user/neo.ts';

/** Where a chat belongs: a server's chat, or the account's (`serverUuid` null, key `account`). */
export interface NeoPlace {
  key: string;
  serverUuid: string | null;
  serverName: string | null;
}

export type NeoView = 'chat' | 'settings' | 'history';

/** A message to send: typed in the chat, or handed over by a page (the log check's "Ask neo"). */
export interface NeoAsk {
  text: string;
  images: string[];
  seed?: NeoSeed | null;
  /** Starts a chat of its own: what neo is doing stops and the open chat goes into the history. */
  fresh?: boolean;
}

/** Something the chat says outside the conversation: a code it translates, or the server's words. */
export interface NeoFlash {
  code: string | null;
  params: Record<string, unknown> | null;
  message: string | null;
}

export interface NeoChat {
  /** The chat's GET (blocked, preferences, links, limits); its conversation lives in `conversation`. */
  info: Omit<NeoInfo, 'conversation'> | null;
  conversation: NeoConversation | null;
  loading: boolean;
  /** Why the chat could not be loaded. */
  failed: string | null;
  /** The loop runs. */
  working: boolean;
  /** Stop was pressed; the step in flight finishes first. */
  stopping: boolean;
  /** The message on its way, shown until the conversation has it. */
  sending: NeoAsk | null;
  /** The answer while the model writes it. */
  liveText: string;
  liveThinking: string;
  /** How a turn ended while nobody looked: shown on the launcher. */
  attention: 'done' | 'approve' | null;
  flash: NeoFlash | null;
  draft: string;
  /** Images (JPEG data URLs) waiting to go with the next message. */
  images: string[];
  /** A page's message waiting for the chat to load, the notice to be agreed to or the turn to end. */
  pending: NeoAsk | null;
}

const emptyChat: NeoChat = {
  info: null,
  conversation: null,
  loading: false,
  failed: null,
  working: false,
  stopping: false,
  sending: null,
  liveText: '',
  liveThinking: '',
  attention: null,
  flash: null,
  draft: '',
  images: [],
  pending: null,
};

export const NEO_ACCOUNT_PLACE: NeoPlace = { key: 'account', serverUuid: null, serverName: null };

interface NeoStore {
  open: boolean;
  view: NeoView;
  /** The chat of the page the person is on; null where neo is not offered. */
  place: NeoPlace | null;
  chats: Record<string, NeoChat>;
  /** A start gate's page a tool sent the person to; the chat navigates there. */
  redirect: string | null;

  setPlace: (place: NeoPlace | null) => void;
  setOpen: (open: boolean) => void;
  setView: (view: NeoView) => void;
  setDraft: (key: string, draft: string) => void;
  setImages: (key: string, images: string[]) => void;
  setFlash: (key: string, flash: NeoFlash | null) => void;
  clearRedirect: () => void;
}

// the chat stays open across page loads (a per-browser convenience, like the Pelican panel's)
const OPEN_KEY = 'neo-chat:open';

function readOpen(): boolean {
  try {
    return window.localStorage.getItem(OPEN_KEY) === '1';
  } catch {
    return false;
  }
}

export const useNeoStore = create<NeoStore>((set) => ({
  open: readOpen(),
  view: 'chat',
  place: null,
  chats: {},
  redirect: null,

  setPlace: (place) => set({ place }),
  setOpen: (open) => set({ open, view: 'chat' }),
  setView: (view) => set({ view }),
  setDraft: (key, draft) => patch(key, { draft }),
  setImages: (key, images) => patch(key, { images }),
  setFlash: (key, flash) => patch(key, { flash }),
  clearRedirect: () => set({ redirect: null }),
}));

useNeoStore.subscribe((state, previous) => {
  if (state.open === previous.open) return;

  try {
    window.localStorage.setItem(OPEN_KEY, state.open ? '1' : '0');
  } catch {
    // private windows and blocked storage: the chat only forgets that it was open
  }
});

export function getNeoStore() {
  return useNeoStore.getState();
}

export function getNeoChat(key: string): NeoChat {
  return useNeoStore.getState().chats[key] ?? emptyChat;
}

/** A place's chat as a hook; an unknown place reads as an empty chat. */
export function useNeoChat<T>(key: string | null | undefined, select: (chat: NeoChat) => T): T {
  return useNeoStore((state) => select((key && state.chats[key]) || emptyChat));
}

function patch(key: string, change: Partial<NeoChat> | ((chat: NeoChat) => Partial<NeoChat>)) {
  useNeoStore.setState((state) => {
    const chat = state.chats[key] ?? emptyChat;
    const next = typeof change === 'function' ? change(chat) : change;

    return { chats: { ...state.chats, [key]: { ...chat, ...next } } };
  });
}

// Each running loop's handle: Stop sets `stop`, and `wake` ends a busy wait early.
const controls = new Map<string, { stop: boolean; wake: (() => void) | null }>();

// the most steps one loop takes before it waits for the person (the backend's own limit is lower)
const MAX_STEPS = 200;
// the longest a busy answer makes the chat wait before it asks again
const MAX_WAIT_SECONDS = 10;

function wait(control: { wake: (() => void) | null }, ms: number): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(done, ms);

    function done() {
      clearTimeout(timer);
      control.wake = null;
      resolve();
    }

    control.wake = done;
  });
}

function errorFlash(error: unknown): NeoFlash {
  // without a response the request never got an answer: the connection is the problem
  return getHttpStatus(error) === null
    ? { code: 'connection', params: null, message: null }
    : { code: null, params: null, message: httpErrorToHuman(error) };
}

/** A result-level reason the conversation does not show as a notice row already. */
function flashFor(result: NeoStepResult, conversation: NeoConversation | null): NeoFlash | null {
  if (!result.code || !['error', 'limited', 'unlinked'].includes(result.state)) {
    return null;
  }

  const last = conversation?.messages.at(-1);
  if (last?.role === 'notice' && last.code === result.code) {
    return null;
  }

  return { code: result.code, params: result.params, message: null };
}

/** Takes in what an action answered: the conversation, a block, a redirect, a reason. */
function apply(key: string, response: NeoStepResponse | NeoStreamResult) {
  const { result, conversation } = response;

  patch(key, (chat) => ({
    conversation: conversation ?? chat.conversation,
    info:
      result.state === 'blocked' && chat.info && result.code
        ? { ...chat.info, blocked: result.code as NonNullable<NeoInfo['blocked']> }
        : chat.info,
    flash: flashFor(result, conversation ?? chat.conversation) ?? chat.flash,
  }));

  if (result.state === 'redirect' && result.url) {
    useNeoStore.setState({ redirect: result.url, open: false });
  }
}

/** Reads the open conversation again after a step whose view could not be built. */
async function refresh(place: NeoPlace) {
  const conversation = getNeoChat(place.key).conversation;
  if (!conversation) return;

  try {
    patch(place.key, { conversation: await getNeoConversation(place.serverUuid, conversation.uuid) });
  } catch {
    // the next action brings it along
  }
}

/**
 * The loop: `first` (send, approve, retry, …), then a streamed step while the answer says
 * `continue`, after a pause while it says `busy`. Everything else ends it until the person acts.
 * Without `first` it resumes a turn that was running when the chat was opened.
 */
async function run(place: NeoPlace, first: (() => Promise<NeoStepResponse>) | null) {
  const key = place.key;
  if (getNeoChat(key).working) return;

  const control = { stop: false, wake: null as (() => void) | null };
  controls.set(key, control);
  patch(key, { working: true, stopping: false, flash: null, liveText: '', liveThinking: '' });

  // deltas come in by the token: they reach the store once a frame
  let text = '';
  let thinking = '';
  let frame: number | null = null;
  const flush = () => {
    frame = null;
    patch(key, { liveText: text, liveThinking: thinking });
  };
  const schedule = () => {
    if (frame === null) frame = requestAnimationFrame(flush);
  };

  let result: NeoStepResult | null = null;

  try {
    if (first) {
      const response = await first();
      apply(key, response);
      result = response.result;
    }

    for (let step = 0; step < MAX_STEPS; step++) {
      if (result && result.state !== 'continue' && result.state !== 'busy') break;
      if (control.stop) break;

      if (result?.state === 'busy') {
        await wait(control, Math.min(Math.max(result.retryAfter, 1), MAX_WAIT_SECONDS) * 1000);
        if (control.stop) break;
      }

      text = '';
      thinking = '';
      const response = await stepNeo(place.serverUuid, {
        onText: (delta) => {
          text += delta;
          schedule();
        },
        onThinking: (delta) => {
          thinking += delta;
          schedule();
        },
        onReset: () => {
          text = '';
          thinking = '';
          schedule();
        },
      });

      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
      patch(key, { liveText: '', liveThinking: '' });

      apply(key, response);
      result = response.result;
      if (!response.conversation) {
        await refresh(place);
      }
    }

    // Stop pressed while a step ran: that step's calls may have queued more work, so cancel
    // again now that it is stored
    if (control.stop && result && (result.state === 'continue' || result.state === 'busy')) {
      const response = await cancelNeo(place.serverUuid);
      apply(key, response);
      result = response.result;
    }
  } catch (error) {
    patch(key, { flash: errorFlash(error) });
    await refresh(place);
  } finally {
    if (frame !== null) cancelAnimationFrame(frame);
    controls.delete(key);

    const { open, place: current } = useNeoStore.getState();
    const watched = open && current?.key === key;

    patch(key, (chat) => ({
      working: false,
      stopping: false,
      liveText: '',
      liveThinking: '',
      attention: watched || !result ? chat.attention : result.state === 'awaiting_approval' ? 'approve' : 'done',
    }));
  }

  // a page's message that came in while the turn ran goes now
  const { pending, info } = getNeoChat(key);
  if (pending && info && !info.blocked) {
    void sendNeo(place, pending);
  }
}

/** Loads a place's chat (on opening it), resumes a running turn and sends a waiting message. */
export async function loadNeo(place: NeoPlace) {
  const chat = getNeoChat(place.key);
  if (chat.loading || chat.working) return;

  patch(place.key, { loading: true, failed: null, attention: null });

  let neo: NeoInfo;
  try {
    neo = await getNeo(place.serverUuid);
  } catch (error) {
    patch(place.key, { loading: false, failed: httpErrorToHuman(error) });
    return;
  }

  const { conversation, ...info } = neo;
  patch(place.key, { loading: false, info, conversation });

  if (info.blocked) return;

  if (conversation.status === 'running') {
    void run(place, null);
    // a page's fresh start stops the turn (the loop sends it when it has ended)
    if (getNeoChat(place.key).pending?.fresh) stopNeo(place);
    return;
  }

  const { pending } = getNeoChat(place.key);
  if (pending) {
    void sendNeo(place, pending);
  }
}

/**
 * Sends a message. One typed in the chat (`typed`) leaves the input at once and comes back to it
 * when it was not taken (blocked, limited, too fast, a turn already running); a page's message
 * waits instead while the chat is busy or the notice is not agreed to yet.
 */
export async function sendNeo(place: NeoPlace, ask: NeoAsk, typed = false) {
  const key = place.key;

  if (getNeoChat(key).working) {
    if (!typed) {
      patch(key, { pending: ask });
      // a fresh start does not wait for the turn: it stops it, and goes once the loop has ended
      if (ask.fresh) stopNeo(place);
    }
    return;
  }

  patch(key, { pending: null, sending: ask, ...(typed ? { draft: '', images: [] } : {}) });

  const current = getNeoChat(key).conversation;
  if (ask.fresh && current && current.messages.length > 0) {
    try {
      // nothing of the old request may still run or wait for an approval
      if (current.status === 'running' || current.status === 'awaiting_approval') {
        await cancelNeo(place.serverUuid);
      }
      patch(key, { conversation: await newNeoChat(place.serverUuid) });
    } catch (error) {
      patch(key, { sending: null, flash: errorFlash(error) });
      return;
    }
  }

  const giveBack = () =>
    patch(key, (chat) =>
      chat.draft === '' && chat.images.length === 0 ? { draft: ask.text, images: ask.images } : {},
    );

  await run(place, async () => {
    try {
      const response = await sendNeoMessage(place.serverUuid, ask);
      if (response.result.state !== 'continue') {
        if (typed) giveBack();
        // not taken: blocked (the notice first) or a turn already running; it goes once that ends
        else if (response.result.state === 'blocked' || response.result.state === 'busy') patch(key, { pending: ask });
      }

      return response;
    } catch (error) {
      if (typed) giveBack();
      throw error;
    } finally {
      patch(key, { sending: null });
    }
  });
}

/** Opens the chat of the page the person is on, optionally with a message to send. */
export function openNeo(ask?: NeoAsk) {
  const { place } = useNeoStore.getState();
  if (!place) return;

  if (ask) patch(place.key, { pending: ask });
  useNeoStore.setState({ open: true, view: 'chat' });
  if (ask?.fresh && getNeoChat(place.key).working) stopNeo(place);
  void loadNeo(place);
}

export function stopNeo(place: NeoPlace) {
  const control = controls.get(place.key);
  if (!control) return;

  control.stop = true;
  control.wake?.();
  patch(place.key, { stopping: true });

  // nothing still waiting runs; the loop cancels once more after the step in flight
  cancelNeo(place.serverUuid).catch(() => undefined);
}

export function approveNeoCall(place: NeoPlace, message: string | null) {
  return run(place, () => approveNeo(place.serverUuid, message));
}

export function denyNeoCall(place: NeoPlace, message: string) {
  return run(place, () => denyNeo(place.serverUuid, message));
}

export function retryNeoTurn(place: NeoPlace) {
  return run(place, () => retryNeo(place.serverUuid));
}

async function swapConversation(place: NeoPlace, load: () => Promise<NeoConversation>) {
  if (getNeoChat(place.key).working) return;

  try {
    const conversation = await load();
    patch(place.key, { conversation, flash: null, attention: null });
    useNeoStore.setState({ view: 'chat' });

    if (conversation.status === 'running') {
      void run(place, null);
    }
  } catch (error) {
    patch(place.key, { flash: errorFlash(error) });
    useNeoStore.setState({ view: 'chat' });
  }
}

/** Puts the open chat into the history and starts an empty one. */
export function newNeoConversation(place: NeoPlace) {
  return swapConversation(place, () => newNeoChat(place.serverUuid));
}

/** Continues an earlier chat. */
export function reopenNeoConversation(place: NeoPlace, conversation: string) {
  return swapConversation(place, () => openNeoConversation(place.serverUuid, conversation));
}

/** Deletes one of this place's chats; the open one makes way for a new, empty chat. */
export async function removeNeoConversation(place: NeoPlace, conversation: string) {
  await deleteNeoConversation(place.serverUuid, conversation);

  if (getNeoChat(place.key).conversation?.uuid === conversation) {
    await loadNeo(place);
  }
}

/** Changes the person's switches; they hold for every place, so each chat takes them in. */
export async function updateNeo(changes: NeoPreferencesUpdate) {
  const preferences = await updateNeoPreferences(changes);

  useNeoStore.setState((state) => ({
    chats: Object.fromEntries(
      Object.entries(state.chats).map(([key, chat]) => [
        key,
        chat.info ? { ...chat, info: { ...chat.info, preferences } } : chat,
      ]),
    ),
  }));

  // turning neo on or off and agreeing to the notice change whether each chat is blocked
  // (the other places load again when they are opened next)
  if (changes.enabled !== undefined || changes.acknowledged) {
    const { place } = useNeoStore.getState();
    useNeoStore.setState((state) => ({
      chats: Object.fromEntries(
        Object.entries(state.chats).map(([key, chat]) => [
          key,
          chat.working || key === place?.key ? chat : { ...chat, info: null },
        ]),
      ),
    }));
    if (place) await loadNeo(place);
  }

  return preferences;
}

/** Deletes every chat of the account; the chat of this page starts again empty. */
export async function deleteAllNeoChats() {
  const deleted = await deleteNeoChats();

  useNeoStore.setState((state) => ({
    chats: Object.fromEntries(
      Object.entries(state.chats).map(([key, chat]) => [key, { ...emptyChat, draft: chat.draft, images: chat.images }]),
    ),
  }));

  const { place } = useNeoStore.getState();
  if (place) await loadNeo(place);

  return deleted;
}
