// plugged.host: the server settings pages behind the jump bar, one per section of the Pelican
// panel's settings page; "Behaviour" holds Calagopus' own auto-start, timezone, auto-kill and
// trash settings, which Pelican has no section for (docs/features/server-settings.md)
import BehaviourContainer from './BehaviourContainer.tsx';
import DangerZoneContainer from './DangerZoneContainer.tsx';
import NotificationsContainer from './NotificationsContainer.tsx';
import ResizeContainer from './ResizeContainer.tsx';
import ServerSettingsShell from './ServerSettingsShell.tsx';
import SftpContainer from './SftpContainer.tsx';
import SoftwareContainer from './SoftwareContainer.tsx';

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
      <BehaviourContainer />
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
