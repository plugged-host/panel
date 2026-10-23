import { QueryKey, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { useOnlineWait } from '@/plugins/useOnlineWait.ts';

const UPGRADE_RESTART_TIMEOUT_MS = 90_000;

export function useUpgradeRestart(onlineWait: ReturnType<typeof useOnlineWait>, queryKeys: QueryKey[]) {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (onlineWait.status !== 'connected' && onlineWait.status !== 'timeout') return;

    for (const queryKey of queryKeys) {
      queryClient.invalidateQueries({ queryKey });
    }
  }, [onlineWait.status]);

  return {
    restarting: onlineWait.status === 'waiting',
    start: (uuid: string, previousVersion: string | null) =>
      onlineWait.start(uuid, UPGRADE_RESTART_TIMEOUT_MS, { untilVersionChangesFrom: previousVersion }),
  };
}
