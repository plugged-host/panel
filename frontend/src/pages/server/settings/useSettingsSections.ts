// plugged.host: which sections of the server's settings page a person gets, shared by the page, its
// jump bar and the sections themselves (docs/features/server-settings.md)
import getServerResize from '@/api/server/settings/getServerResize.ts';
import getServerSoftware from '@/api/server/settings/getServerSoftware.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

// the page's sections by their anchor (/settings#sftp), the Pelican panel's section ids
// (server-form.blade.php); Behaviour is ours
export type SettingsSection = 'general' | 'sftp' | 'resources' | 'version' | 'behaviour' | 'notifications' | 'danger';

// a section's TitleCard props: its anchor, and the class that stops it below the sticky jump bar
export function sectionAnchor(id: SettingsSection) {
  return { id, className: 'theme-jump-target' };
}

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

// whether Behaviour has a part for the person: auto-start, timezone, auto-kill, or the trash where the
// panel lets servers change it (BehaviourContainer.tsx)
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
