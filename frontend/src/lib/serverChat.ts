// plugged.host: the server chat's thread as it is drawn (docs/features/server-chat.md): its pages as one
// list, the list in days and each day in runs of one person's messages, and the labels of days and times

/** One person's messages this close together share one name and time. */
export const CHAT_RUN_GAP_MS = 5 * 60 * 1000;

interface ChatEntry {
  uuid: string;
  user: { uuid: string } | null;
  created: Date;
}

/** Messages of one person, close together, under one name and time. */
export interface ChatRun<T extends ChatEntry> {
  /** The first message's uuid. */
  key: string;
  own: boolean;
  user: T['user'];
  /** The first message's time. */
  created: Date;
  messages: T[];
}

/** The runs of one local calendar day, under its label. */
export interface ChatDay<T extends ChatEntry> {
  key: string;
  date: Date;
  runs: ChatRun<T>[];
}

/**
 * The loaded pages (newest page first, each page oldest to newest) as one list, oldest first, each
 * message once: while new messages push older ones onto the next page, two pages can hold the same
 * message until both are fetched again.
 */
export function mergeChatPages<T extends { uuid: string; created: Date }>(pages: T[][]): T[] {
  const byUuid = new Map<string, T>();
  for (let index = pages.length - 1; index >= 0; index--) {
    for (const message of pages[index]) {
      byUuid.set(message.uuid, message);
    }
  }

  // sort is stable: messages of the same moment keep the order the API gave them
  return [...byUuid.values()].sort((a, b) => a.created.getTime() - b.created.getTime());
}

function dayKey(date: Date): string {
  return `${date.getFullYear()}-${date.getMonth() + 1}-${date.getDate()}`;
}

/**
 * Messages (oldest first) in days, each day in runs: a run is one person's messages, each at most
 * `CHAT_RUN_GAP_MS` after the one before. A deleted account's messages never join a run, as they
 * may have been different people.
 */
export function groupChat<T extends ChatEntry>(messages: T[], ownUuid: string | null | undefined): ChatDay<T>[] {
  const days: ChatDay<T>[] = [];
  let day: ChatDay<T> | null = null;
  let run: ChatRun<T> | null = null;
  let previous: T | null = null;

  for (const message of messages) {
    const key = dayKey(message.created);
    if (!day || day.key !== key) {
      day = { key, date: message.created, runs: [] };
      days.push(day);
      run = null;
    }

    const author = message.user?.uuid ?? null;
    const gap = previous ? message.created.getTime() - previous.created.getTime() : Infinity;
    if (!run || author === null || run.user?.uuid !== author || gap < 0 || gap > CHAT_RUN_GAP_MS) {
      run = {
        key: message.uuid,
        own: author !== null && author === ownUuid,
        user: message.user,
        created: message.created,
        messages: [],
      };
      day.runs.push(run);
    }

    run.messages.push(message);
    previous = message;
  }

  return days;
}

const relativeFormats = new Map<string, Intl.RelativeTimeFormat>();

function relativeFormat(locale: string): Intl.RelativeTimeFormat {
  let format = relativeFormats.get(locale);
  if (!format) {
    format = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
    relativeFormats.set(locale, format);
  }

  return format;
}

function capitalize(text: string, locale: string): string {
  return text.charAt(0).toLocaleUpperCase(locale) + text.slice(1);
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/**
 * A day's label in `locale`: Today and Yesterday, the weekday within the last week, else the date,
 * with the year when it is not this one. A day after `now` (a clock running ahead) counts as today.
 */
export function chatDayLabel(date: Date, now: Date, locale: string): string {
  // rounded: a day with a daylight saving change is 23 or 25 hours long
  const days = Math.max(0, Math.round((startOfDay(now) - startOfDay(date)) / 86_400_000));

  if (days <= 1) {
    return capitalize(relativeFormat(locale).format(-days, 'day'), locale);
  }
  if (days < 7) {
    return capitalize(date.toLocaleDateString(locale, { weekday: 'long' }), locale);
  }

  return capitalize(
    date.toLocaleDateString(locale, {
      day: 'numeric',
      month: 'long',
      year: date.getFullYear() === now.getFullYear() ? undefined : 'numeric',
    }),
    locale,
  );
}

/**
 * A message's time in `locale`: "now" and minutes ago within the hour, after that the time of day
 * (the day is in its label above).
 */
export function chatTimeLabel(date: Date, now: Date, locale: string): string {
  const minutes = Math.floor((now.getTime() - date.getTime()) / 60_000);

  if (minutes < 1) {
    return relativeFormat(locale).format(0, 'second');
  }
  if (minutes < 60) {
    return relativeFormat(locale).format(-minutes, 'minute');
  }

  return date.toLocaleTimeString(locale, { hour: 'numeric', minute: '2-digit' });
}
