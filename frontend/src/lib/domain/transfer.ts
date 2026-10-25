// plugged.host: a server's move as the Transfer page and the strip word it (docs/features/transfers.md)
import { z } from 'zod';
import {
  activeTransferStatuses,
  serverTransferEventSchema,
  serverTransferRequestSchema,
} from '@/lib/schemas/server/transfer.ts';
import { getTranslations, useTranslations } from '@/providers/TranslationProvider.tsx';

type Request = z.infer<typeof serverTransferRequestSchema>;
type Event = z.infer<typeof serverTransferEventSchema>;
type Translate = ReturnType<typeof useTranslations>['t'];

const STATES = [
  'queued',
  'preparing',
  'stopping',
  'copying',
  'finalizing',
  'starting',
  'retrying',
  'checking',
  'cancelling',
  'completed',
  'failed',
  'cancelled',
] as const;
const EVENTS = [
  'requested',
  'adopted',
  'destination',
  'retrying',
  'attemptFailed',
  'copied',
  'portVariable',
  'completed',
  'endedFailed',
  'cancelling',
  'endedCancelled',
  'started',
  'startFailed',
  'restarted',
  'restartFailed',
] as const;
const REASONS = ['noNode', 'copyFailed', 'skippedMoved'] as const;
const BLOCKED = ['disabled', 'busy', 'suspended', 'installing', 'restoring', 'backupRunning', 'cooldown'] as const;

/** The stepper's steps, in order (backups travel inside the copy, so they have none of their own). */
export const TRANSFER_STEPS = ['prepare', 'stop', 'copy', 'verify', 'start'] as const;

/** How long the Transfer page shows the last move's outcome, and the strip, in minutes (server_transfers.rs). */
export const OUTCOME_MINUTES = 10;

const known = <T extends string>(list: readonly T[], value: string | null | undefined): T | null =>
  value && (list as readonly string[]).includes(value) ? (value as T) : null;

export function isActiveTransfer(request: Request | null | undefined): request is Request {
  return !!request && (activeTransferStatuses as readonly string[]).includes(request.status);
}

/** A finished move that ended less than `minutes` ago. */
export function endedWithin(request: Request | null | undefined, minutes: number): boolean {
  return !!request?.finished && Date.now() - request.finished.getTime() < minutes * 60_000;
}

/** "2m 10s": the two largest units, like the Pelican panel's short durations. */
export function shortDuration(seconds: number): string {
  const total = Math.max(1, Math.round(seconds));
  const parts = {
    days: Math.floor(total / 86400),
    hours: Math.floor((total % 86400) / 3600),
    minutes: Math.floor((total % 3600) / 60),
    seconds: total % 60,
  };
  const units = ['days', 'hours', 'minutes', 'seconds'] as const;
  const first = units.findIndex((unit) => parts[unit] > 0);
  const kept = Object.fromEntries(units.slice(first, first + 2).map((unit) => [unit, parts[unit]]));

  return new Intl.DurationFormat(getTranslations().language, { style: 'narrow' }).format(kept);
}

/** The headline of a move's state ("Copying files", "Moved"). */
export function transferStateText(t: Translate, state: string): string {
  return t(`pages.server.transfer.state.${known(STATES, state) ?? 'preparing'}`, {});
}

/** Why a move ended, translated when the panel named the reason, else as the backend wrote it. */
export function transferReasonText(t: Translate, request: Request): string | null {
  const reason = known(REASONS, request.reason);

  return reason ? t(`pages.server.transfer.reasons.${reason}`, {}) : request.error;
}

/** A line of a move's timeline; `null` for a line this panel does not know. */
export function transferEventText(t: Translate, event: Event): string | null {
  const key = known(EVENTS, event.key);
  if (!key) return null;

  const reason = known(REASONS, event.params.reason);

  return t(`pages.server.transfer.events.${key}`, {
    location: event.params.location ?? '',
    attempt: event.params.attempt ?? '',
    max: event.params.max ?? '',
    reason: reason ? t(`pages.server.transfer.reasons.${reason}`, {}) : (event.params.reason ?? ''),
    address: event.params.address ?? '',
    name: event.params.name ?? '',
    port: event.params.port ?? '',
  });
}

/** Why a move cannot start now; `null` when it can. */
export function transferBlockedText(t: Translate, blocked: string | null, cooldownUntil: Date | null): string | null {
  const key = known(BLOCKED, blocked);
  if (!key) return null;

  return t(`pages.server.transfer.blocked.${key}`, {
    time: shortDuration(Math.max(0, ((cooldownUntil?.getTime() ?? Date.now()) - Date.now()) / 1000)),
  });
}
