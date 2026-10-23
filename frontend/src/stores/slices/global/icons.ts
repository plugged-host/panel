import { z } from 'zod';
import { StateCreator } from 'zustand';
import { iconSchema } from '@/lib/schemas/icons.ts';
import { GlobalStore } from '@/stores/global.ts';

export interface IconsSlice {
  icons: Record<string, z.infer<typeof iconSchema>>;

  setIcons: (icons: Record<string, z.infer<typeof iconSchema>>) => void;
  addIcons: (icons: Record<string, z.infer<typeof iconSchema>>) => void;
}

export const createIconsSlice: StateCreator<GlobalStore, [], [], IconsSlice> = (set): IconsSlice => ({
  icons: {},

  setIcons: (value) => set((state) => ({ ...state, icons: value })),
  addIcons: (value) => set((state) => ({ ...state, icons: { ...state.icons, ...value } })),
});
