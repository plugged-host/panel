import { useServerStore } from '@/stores/server.ts';

export function useServerStatsPending() {
  return useServerStore(
    (state) => state.stats === null && !state.server.nodeMaintenanceEnabled && state.socketError?.recoverable !== false,
  );
}
