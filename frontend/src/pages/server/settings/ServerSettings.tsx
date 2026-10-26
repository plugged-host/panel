import { useState } from 'react'; // plugged.host
import Button from '@/elements/buttons/Button.tsx'; // plugged.host
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx'; // plugged.host
import { useServerCan } from '@/plugins/usePermissions.ts'; // plugged.host
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import BehaviourContainer from './BehaviourContainer.tsx'; // plugged.host
import DangerZoneContainer from './DangerZoneContainer.tsx'; // plugged.host
import DeleteServerModal from './modals/DeleteServerModal.tsx'; // plugged.host
import NotificationsContainer from './NotificationsContainer.tsx'; // plugged.host
import ResizeContainer from './ResizeContainer.tsx'; // plugged.host
import ServerInformationContainer from './ServerInformationContainer.tsx'; // plugged.host
import ServerSettingsNav from './ServerSettingsNav.tsx'; // plugged.host
import SftpContainer from './SftpContainer.tsx'; // plugged.host
import SoftwareContainer from './SoftwareContainer.tsx'; // plugged.host
import { useBehaviourShown, useCanDeleteServer, useResizeOptions, useSoftwareOptions } from './useSettingsSections.ts'; // plugged.host

// plugged.host: one page, the Pelican panel's settings page (Theme v2's server-form.blade.php): the
// title with a gray Delete Server (its header action; the red is in the confirmation), the jump bar,
// then every section in one column in Pelican's order, each only for those who may use it.
// Upstream's cards (Debug Information, Rename, Auto-Kill, Auto-Start, Timezone, Trash, Reinstall)
// are unused; Startup and VS Code are pages of their own (docs/features/server-settings.md,
// docs/upstream.md)
export default function ServerSettings() {
  const { t } = useTranslations();
  // plugged.host
  const canSftp = useServerCan('files.sftp');
  const canBehaviour = useBehaviourShown();
  const canNotifications = useServerCan('backups.read');
  const canDelete = useCanDeleteServer();
  const resize = useResizeOptions();
  const software = useSoftwareOptions();
  const [deleting, setDeleting] = useState(false);
  // the page waits for the two answers that decide whether Resources and Version are on it, so the
  // sections below them do not move when they arrive (Pelican renders the page at once)
  const settling =
    (resize.resource.loading && resize.resource.data === undefined) ||
    (software.resource.loading && software.resource.data === undefined);

  return (
    <ServerContentContainer
      title={t('pages.server.settings.title', {})}
      registry={window.extensionContext.extensionRegistry.pages.server.settings.container}
      contentRight={
        canDelete ? (
          <Button variant='default' onClick={() => setDeleting(true)}>
            {t('pages.server.settings.delete.button', {})}
          </Button>
        ) : undefined
      }
    >
      {canDelete && <DeleteServerModal opened={deleting} onClose={() => setDeleting(false)} />}

      {/* plugged.host: the sections in one column, Filament's 1.5rem apart, instead of the card grid */}
      {settling ? (
        <Spinner.Centered />
      ) : (
        <div className='flex flex-col gap-6'>
          <ServerSettingsNav />

          <ExtensionSlot
            components={
              window.extensionContext.extensionRegistry.pages.server.settings.settingContainers.prependedComponents
            }
            name='settings-settingContainer-prepended'
          />

          <ServerInformationContainer />
          {canSftp && <SftpContainer />}
          <ResizeContainer />
          <SoftwareContainer />
          {canBehaviour && <BehaviourContainer />}
          {canNotifications && <NotificationsContainer />}

          <ExtensionSlot
            components={
              window.extensionContext.extensionRegistry.pages.server.settings.settingContainers.appendedComponents
            }
            name='settings-settingContainer-appended'
          />

          {/* Pelican's last section */}
          <DangerZoneContainer />
        </div>
      )}
    </ServerContentContainer>
  );
}
