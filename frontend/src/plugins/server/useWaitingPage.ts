// plugged.host: what the start queue and the waiting room share: a clock that moves every second
// between polls, the way out, and the waiting room's hold on the page (docs/features/start-queue.md,
// docs/features/ads.md#waiting-room)
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { adScriptsLoaded } from '@/elements/ads/AdScripts.tsx';
import { useBlocker } from '@/plugins/useBlocker.ts';

/**
 * A 1 Hz clock re-seeded from the server's `base` seconds on every poll: the browser counts
 * between polls, the server decides. `onZero` runs once when `max` is reached, so the release
 * the page promised is asked for at that moment rather than at the next poll.
 */
export function useWaitClock(base: number, max: number, onZero?: () => void) {
  const [seed, setSeed] = useState(base);
  const [ticks, setTicks] = useState(0);
  if (seed !== base) {
    setSeed(base);
    setTicks(0);
  }

  useEffect(() => {
    const interval = setInterval(() => setTicks((ticks) => ticks + 1), 1_000);
    return () => clearInterval(interval);
  }, []);

  const elapsed = base + (seed === base ? ticks : 0);
  const remaining = Math.max(0, max - elapsed);
  const percent = max > 0 ? Math.min(100, (elapsed / max) * 100) : 100;

  const nudged = useRef(false);
  useEffect(() => {
    if (remaining > 0 || nudged.current) return;
    nudged.current = true;
    onZero?.();
  }, [remaining]);

  return { elapsed, remaining, percent };
}

/** "4:07": minutes and seconds waited. */
export function formatWaited(seconds: number): string {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

/**
 * Leaves a waiting page for `path`, replacing it in the history: a full load when page-level ad
 * scripts ran here, so they end with the page.
 */
export function useLeaveWaitingPage() {
  const navigate = useNavigate();

  return (path: string) => {
    if (adScriptsLoaded()) {
      window.location.assign(path);
    } else {
      navigate(path, { replace: true });
    }
  };
}

/**
 * While `active`, the page stays: the back button and the panel's own links do nothing, and the
 * browser asks before a reload or closing the tab.
 */
export function useHoldPage(active: boolean) {
  const blocker = useBlocker(active);

  useEffect(() => {
    if (blocker.state === 'blocked') blocker.reset();
  }, [blocker.state]);
}
