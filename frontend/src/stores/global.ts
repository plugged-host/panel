import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { AnnouncementsSlice, createAnnouncementsSlice } from './slices/global/announcements.ts';
import { createIconsSlice, IconsSlice } from './slices/global/icons.ts';
import { createPermissionsSlice, PermissionsSlice } from './slices/global/permissions.ts';
import { createSettingsSlice, SettingsSlice } from './slices/global/settings.ts';
import { createTimeSlice, TimeSlice } from './slices/global/time.ts';

export interface GlobalStore extends AnnouncementsSlice, IconsSlice, PermissionsSlice, SettingsSlice, TimeSlice {}

export const useGlobalStore = create<GlobalStore>()(
  persist(
    (...a) => ({
      ...createAnnouncementsSlice(...a),
      ...createIconsSlice(...a),
      ...createPermissionsSlice(...a),
      ...createSettingsSlice(...a),
      ...createTimeSlice(...a),
    }),
    { name: 'global' },
  ),
);

export function getGlobalStore() {
  return useGlobalStore.getState();
}
