// plugged.host: what the server's Settings page shows a person and the answers it waits for, shared by
// the page and its sections; useCanDeleteServer also by InstallStuck and ServerStateGuard
// (docs/features/server-settings.md)
import getServerNotifications from '@/api/server/settings/getServerNotifications.ts';
import getServerResize from '@/api/server/settings/getServerResize.ts';
import getServerSoftware from '@/api/server/settings/getServerSoftware.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

// the page's sections by their anchor (/settings#sftp), where the old /settings/<section> addresses land
export const SETTINGS_SECTIONS = [
  'general',
  'sftp',
  'resources',
  'upgrades', // docs/features/server-upgrades.md
  'version',
  'behaviour',
  'notifications',
  'danger',
] as const;

export type SettingsSectionId = (typeof SETTINGS_SECTIONS)[number];

// what the server may be resized to; `usable` once the person may resize it
export function useResizeOptions() {
  const { user } = useAuth();
  const server = useServerStore((state) => state.server);
  const allowResizing = useGlobalStore((state) => state.settings.games.allowResizing);
  const canResize = useServerCan('settings.resize');

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).settings.resize(),
    queryFn: () => getServerResize(server.uuid),
    enabled: canResize && (allowResizing || !!user?.admin),
    silent: true,
  });
  const usable = canResize && !!resource.data && (resource.data.enabled || !!user?.admin);

  return { resource, usable };
}

// the software and Minecraft version choices; `usable` when there is something to choose
export function useSoftwareOptions() {
  const server = useServerStore((state) => state.server);
  const canSwitch = useServerCan('settings.software');

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).settings.software(),
    queryFn: () => getServerSoftware(server.uuid),
    enabled: canSwitch,
    silent: true,
  });
  const software = resource.data;
  const usable = canSwitch && !!software && (software.variants.length >= 2 || software.version !== null);

  return { resource, usable };
}

// the person's backup mails for this server; the page waits for them silently, the section says what failed
export function useNotificationSettings(silent = false) {
  const server = useServerStore((state) => state.server);
  const canRead = useServerCan('backups.read');

  return useResource({
    queryKey: queryKeys.server(server.uuid).settings.notifications(),
    queryFn: () => getServerNotifications(server.uuid),
    enabled: canRead,
    silent,
  });
}

// whether Behaviour has a part for the person: auto-start, timezone, auto-kill, or the trash where the
// panel lets servers change it (BehaviourSection.tsx)
export function useBehaviourShown() {
  const allowChangingTrashSettings = useGlobalStore((state) => state.settings.server.allowChangingTrashSettings);
  const canOther = useServerCan(['settings.auto-kill', 'settings.auto-start', 'settings.timezone']);
  const canTrash = useServerCan('settings.trash');

  return canOther || (canTrash && allowChangingTrashSettings);
}

// deleting needs the permission and a panel that lets players delete their servers (admins always may)
export function useCanDeleteServer() {
  const { user } = useAuth();
  const allowDeleting = useGlobalStore((state) => state.settings.games.allowDeleting);
  const canDelete = useServerCan('settings.delete');

  return canDelete && (allowDeleting || !!user?.admin);
}
