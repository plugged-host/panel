// plugged.host: the server settings pages behind the jump bar, one per section of the Pelican
// panel's settings page; "Behaviour" holds Calagopus' own Auto-Kill, Auto-Start, Timezone and
// Trash cards, which Pelican has no section for (docs/features/server-settings.md)
import { ServerCan } from '@/elements/Can.tsx';
import AutokillContainer from './AutokillContainer.tsx';
import AutostartContainer from './AutostartContainer.tsx';
import DangerZoneContainer from './DangerZoneContainer.tsx';
import NotificationsContainer from './NotificationsContainer.tsx';
import ResizeContainer from './ResizeContainer.tsx';
import ServerSettingsShell from './ServerSettingsShell.tsx';
import SftpContainer from './SftpContainer.tsx';
import SoftwareContainer from './SoftwareContainer.tsx';
import TimezoneContainer from './TimezoneContainer.tsx';
import TrashContainer from './TrashContainer.tsx';

export function ServerSettingsSftp() {
  return (
    <ServerSettingsShell>
      <SftpContainer />
    </ServerSettingsShell>
  );
}

export function ServerSettingsResources() {
  return (
    <ServerSettingsShell>
      <ResizeContainer />
    </ServerSettingsShell>
  );
}

export function ServerSettingsVersion() {
  return (
    <ServerSettingsShell>
      <SoftwareContainer />
    </ServerSettingsShell>
  );
}

export function ServerSettingsBehaviour() {
  return (
    <ServerSettingsShell>
      <ServerCan action='settings.auto-kill'>
        <AutokillContainer />
      </ServerCan>
      <ServerCan action='settings.auto-start'>
        <AutostartContainer />
      </ServerCan>
      <ServerCan action='settings.timezone'>
        <TimezoneContainer />
      </ServerCan>
      <ServerCan action='settings.trash'>
        <TrashContainer />
      </ServerCan>
    </ServerSettingsShell>
  );
}

export function ServerSettingsNotifications() {
  return (
    <ServerSettingsShell>
      <NotificationsContainer />
    </ServerSettingsShell>
  );
}

export function ServerSettingsDanger() {
  return (
    <ServerSettingsShell>
      <DangerZoneContainer />
    </ServerSettingsShell>
  );
}
