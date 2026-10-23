import { create } from 'zustand';

export type AssetReloadStatus = 'idle' | 'reloading' | 'offline' | 'stalled';

interface AssetReloadStore {
  status: AssetReloadStatus;
}

const attemptsKey = 'chunkReloadAttempts';
const maxAttempts = 3;
const stalledAfter = 5000;

export const useAssetReloadStore = create<AssetReloadStore>()(() => ({
  status: 'idle',
}));

export function reloadNow() {
  window.location.reload();

  setTimeout(() => useAssetReloadStore.setState({ status: 'stalled' }), stalledAfter);
}

export function scheduleAssetReload() {
  const attempts = parseInt(sessionStorage.getItem(attemptsKey) || '0');
  if (useAssetReloadStore.getState().status !== 'idle' || attempts >= maxAttempts) return;

  useAssetReloadStore.setState({ status: 'reloading' });
  sessionStorage.setItem(attemptsKey, (attempts + 1).toString());

  setTimeout(
    () => {
      if (navigator.onLine) return reloadNow();

      useAssetReloadStore.setState({ status: 'offline' });
      window.addEventListener('online', reloadNow, { once: true });
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible') reloadNow();
      });
    },
    1000 * 2 ** attempts,
  );
}

export function registerAssetReloadHandler() {
  window.addEventListener('vite:preloadError', scheduleAssetReload);

  setTimeout(() => {
    if (useAssetReloadStore.getState().status === 'idle') sessionStorage.removeItem(attemptsKey);
  }, 30_000);
}
