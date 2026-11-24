import { create } from 'zustand';
import { createServersSlice, ServerSlice } from '@/stores/slices/user/servers.ts';

export interface UserStore extends ServerSlice {
  /** plugged.host: the account owns a paid server, so its Paid page stays in the sidebar (docs/features/dedicated.md) */
  paidServers: boolean;
  reset: () => void;
}

export const useUserStore = create<UserStore>()((...a) => {
  const initialState = {} as UserStore;
  Object.assign(initialState, {
    ...createServersSlice(...a),
    paidServers: false, // plugged.host
  });
  initialState.reset = () => a[0]((state) => ({ ...initialState, reset: state.reset }), true);
  return initialState;
});

export function getUserStore() {
  return useUserStore.getState();
}
