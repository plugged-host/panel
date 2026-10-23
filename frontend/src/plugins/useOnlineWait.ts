import { QueryKey, useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

const POLL_INTERVAL_MS = 2_000;

export type OnlineWaitStatus = 'idle' | 'waiting' | 'connected' | 'timeout';

export function useOnlineWait<T extends { version: string }>({
  queryKey,
  queryFn,
}: {
  queryKey: (uuid: string) => QueryKey;
  queryFn: (uuid: string) => Promise<T>;
}) {
  const [uuid, setUuid] = useState<string | null>(null);
  const [timeoutMs, setTimeoutMs] = useState(0);
  const [timedOut, setTimedOut] = useState(false);
  const [previousVersion, setPreviousVersion] = useState<string | null>(null);

  const isOnline = (data: T | undefined) => !!data && (previousVersion === null || data.version !== previousVersion);

  const { data } = useQuery({
    queryKey: [...queryKey(uuid ?? ''), 'online-wait'],
    queryFn: () => queryFn(uuid!),
    enabled: !!uuid && !timedOut,
    retry: false,
    refetchInterval: (query) => (isOnline(query.state.data) ? false : POLL_INTERVAL_MS),
    gcTime: 0,
    meta: { progress: false },
  });

  const online = isOnline(data);

  useEffect(() => {
    if (!uuid || online) return;

    const timeout = setTimeout(() => setTimedOut(true), timeoutMs);
    return () => clearTimeout(timeout);
  }, [uuid, online, timeoutMs]);

  const status: OnlineWaitStatus = !uuid ? 'idle' : online ? 'connected' : timedOut ? 'timeout' : 'waiting';

  return {
    status,
    version: data?.version ?? null,
    start: (uuid: string, timeout: number, options?: { untilVersionChangesFrom?: string | null }) => {
      setTimedOut(false);
      setTimeoutMs(timeout);
      setPreviousVersion(options?.untilVersionChangesFrom ?? null);
      setUuid(uuid);
    },
  };
}
