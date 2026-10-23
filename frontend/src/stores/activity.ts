import { create } from 'zustand';

declare module '@tanstack/react-query' {
  interface Register {
    queryMeta: { progress?: boolean };
  }
}

interface ActivityStore {
  pending: number;
}

export const useActivityStore = create<ActivityStore>()(() => ({
  pending: 0,
}));

export function trackActivity<T>(promise: Promise<T>): Promise<T> {
  useActivityStore.setState((state) => ({ pending: state.pending + 1 }));

  return promise.finally(() => useActivityStore.setState((state) => ({ pending: state.pending - 1 })));
}
