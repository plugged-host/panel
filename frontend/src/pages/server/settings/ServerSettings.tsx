import { useEffect, useRef } from 'react'; // plugged.host
import { useLocation } from 'react-router'; // plugged.host
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx'; // plugged.host
import { useServerCan } from '@/plugins/usePermissions.ts'; // plugged.host
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerUpgrades } from '../upgrades/useServerUpgrades.ts'; // plugged.host
import BehaviourSection from './BehaviourSection.tsx'; // plugged.host
import DangerSection from './DangerSection.tsx'; // plugged.host
import InformationSection from './InformationSection.tsx'; // plugged.host
import NotificationsSection from './NotificationsSection.tsx'; // plugged.host
import ResourcesSection from './ResourcesSection.tsx'; // plugged.host
import SftpSection from './SftpSection.tsx'; // plugged.host
import UpgradesSection from './UpgradesSection.tsx'; // plugged.host
import {
  SETTINGS_SECTIONS,
  useBehaviourShown,
  useNotificationSettings,
  useResizeOptions,
  useSoftwareOptions,
} from './useSettingsSections.ts'; // plugged.host
import VersionSection from './VersionSection.tsx'; // plugged.host

// plugged.host: the server's settings on one page, laid out as v3's own pages are (Transfer, Limbo, the
// account pages): the title, then a column of sections, each a heading on the page with one box under
// it (SettingsSection.tsx), each only for those who may use it. /settings#<section>, where the old
// /settings/<section> addresses land, opens at that section. Upstream's cards (Debug Information,
// Rename, Auto-Kill, Auto-Start, Timezone, Trash, Reinstall) are unused; Startup and VS Code are pages
// of their own (docs/features/server-settings.md, docs/upstream.md)
export default function ServerSettings() {
  const { t } = useTranslations();
  // plugged.host
  const location = useLocation();
  const column = useRef<HTMLDivElement>(null);
  const canSftp = useServerCan('files.sftp');
  const canBehaviour = useBehaviourShown();
  const canNotifications = useServerCan('backups.read');
  const resize = useResizeOptions();
  const software = useSoftwareOptions();
  const notifications = useNotificationSettings(true);
  const upgrades = useServerUpgrades(); // plugged.host: docs/features/server-upgrades.md
  // the page waits for the first answers that decide which sections it has or fill them, so nothing
  // moves once it shows and an address with a section opens at the right place
  const settling =
    (resize.resource.loading && resize.resource.data === undefined) ||
    (software.resource.loading && software.resource.data === undefined) ||
    (notifications.loading && notifications.data === undefined) ||
    (upgrades.resource.loading && upgrades.resource.data === undefined);

  // plugged.host: the section the address names, brought to the top once the page shows; looked up in
  // this page's own column, as a second copy of the page (in a window) has the same ids
  useEffect(() => {
    const id = location.hash.slice(1);
    if (settling || !(SETTINGS_SECTIONS as readonly string[]).includes(id)) return;

    column.current?.querySelector<HTMLElement>(`#${id}`)?.scrollIntoView({ block: 'start' });
  }, [settling, location.hash, location.key]);

  return (
    <ServerContentContainer
      title={t('pages.server.settings.title', {})}
      registry={window.extensionContext.extensionRegistry.pages.server.settings.container}
    >
      {/* plugged.host: the sections in one column, 1.5rem apart, instead of the card grid */}
      {settling ? (
        <Spinner.Centered />
      ) : (
        <div ref={column} className='flex flex-col gap-6'>
          <ExtensionSlot
            components={
              window.extensionContext.extensionRegistry.pages.server.settings.settingContainers.prependedComponents
            }
            name='settings-settingContainer-prepended'
          />

          {/* plugged.host */}
          <InformationSection />
          {canSftp && <SftpSection />}
          <ResourcesSection />
          <UpgradesSection />
          <VersionSection />
          {canBehaviour && <BehaviourSection />}
          {canNotifications && <NotificationsSection />}

          <ExtensionSlot
            components={
              window.extensionContext.extensionRegistry.pages.server.settings.settingContainers.appendedComponents
            }
            name='settings-settingContainer-appended'
          />

          {/* plugged.host: the destructive actions last */}
          <DangerSection />
        </div>
      )}
    </ServerContentContainer>
  );
}
