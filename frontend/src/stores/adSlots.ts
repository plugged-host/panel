// plugged.host: the ad slots on screen, for the allocator in lib/ads.ts (docs/features/ads.md#slots)
import { create } from 'zustand';
import { type AdSlotState } from '@/lib/ads.ts';

interface AdSlotsStore {
  slots: Map<number, AdSlotState>;

  register: (slot: AdSlotState) => void;
  unregister: (id: number) => void;
}

export const useAdSlotsStore = create<AdSlotsStore>()((set) => ({
  slots: new Map(),

  register: (slot) =>
    set((state) => {
      const existing = state.slots.get(slot.id);
      if (existing && existing.width === slot.width && existing.kind === slot.kind) return state;
      return { slots: new Map(state.slots).set(slot.id, slot) };
    }),
  unregister: (id) =>
    set((state) => {
      if (!state.slots.has(id)) return state;
      const slots = new Map(state.slots);
      slots.delete(id);
      return { slots };
    }),
}));
