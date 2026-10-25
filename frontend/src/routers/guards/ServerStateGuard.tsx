import { useEffect, useMemo, useState } from 'react'; // plugged.host: useEffect
import { Navigate, NavLink, Outlet, useLocation } from 'react-router'; // plugged.host: Navigate
import { httpErrorToHuman } from '@/api/axios.ts';
import unlockBackupRestore from '@/api/server/backups/unlockBackupRestore.ts';
import cancelServerInstall from '@/api/server/settings/cancelServerInstall.ts';
import Button from '@/elements/buttons/Button.tsx';
import { ServerCan } from '@/elements/Can.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import ScreenBlock from '@/elements/feedback/ScreenBlock.tsx';
import { DeleteServerButton } from '@/elements/InstallStuck.tsx'; // plugged.host
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { isInLimbo } from '@/lib/domain/limbo.ts'; // plugged.host
import { isConflictingState, serverStatusInfo } from '@/lib/domain/server.ts';
import ServerLimbo from '@/pages/server/limbo/ServerLimbo.tsx'; // plugged.host
import { SETUP_PATH, useMinecraftSetup } from '@/pages/server/minecraft/setup/useMinecraftSetup.ts'; // plugged.host
import { useCanDeleteServer } from '@/pages/server/settings/useSettingsSections.ts'; // plugged.host
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import SuspensionNotice from '@/routers/guards/SuspensionNotice.tsx'; // plugged.host
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

export default function ServerStateGuard() {
  const { t } = useTranslations();
  const settings = useGlobalStore((state) => state.settings);
  const { user } = useAuth();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const canReadInstallationLogs = useAdminCan('servers.read');
  const canUpdateServers = useAdminCan('servers.update');
  const canDeleteServer = useCanDeleteServer(); // plugged.host
  const location = useLocation();

  const [openModal, setOpenModal] = useState<'acknowledgeFailure' | null>(null);
  // plugged.host: a new Minecraft server shows its owner the setup first (docs/features/minecraft.md)
  const setup = useMinecraftSetup(server.isOwner);
  const serverPaths = [`/server/${server.uuid}`, `/server/${server.uuidShort}`];
  const onSetup = serverPaths.some((path) => location.pathname.startsWith(`${path}${SETUP_PATH}`));
  const onPopout = serverPaths.some((path) => location.pathname === `${path}/console/popout`);
  // plugged.host: sent to the setup, says why (the Pelican panel's RequireServerOnboarding notice)
  const toSetup =
    !!setup.data?.pending &&
    !onSetup &&
    !onPopout &&
    !server.isSuspended &&
    !server.isTransferring &&
    !server.nodeMaintenanceEnabled &&
    !isInLimbo(server.limboState);
  useEffect(() => {
    if (toSetup) addToast(t('pages.server.minecraft.setup.gate', {}), 'warning');
  }, [toSetup]);

  const acknowledge = useMemo(() => {
    if (server.status === 'install_failed') {
      if (!settings.server.allowAcknowledgingInstallationFailure && !canUpdateServers) {
        return null;
      }

      return {
        permission: 'settings.cancel-install' as const,
        title: t('elements.screenBlock.serverConflict.modal.acknowledgeInstallFailure.title', {}),
        content: t('elements.screenBlock.serverConflict.modal.acknowledgeInstallFailure.content', {}),
        request: () => cancelServerInstall(server.uuid),
      };
    }

    if (server.status === 'backup_restore_failed') {
      return {
        permission: 'backups.restore' as const,
        title: t('elements.screenBlock.serverConflict.modal.acknowledgeBackupRestoreFailure.title', {}),
        content: t('elements.screenBlock.serverConflict.modal.acknowledgeBackupRestoreFailure.content', {}),
        request: () => unlockBackupRestore(server.uuid).then(() => true),
      };
    }

    return null;
  }, [server.status, server.uuid, settings.server.allowAcknowledgingInstallationFailure, canUpdateServers, t]);

  const doAcknowledgeFailure = async () => {
    await acknowledge
      ?.request()
      .then((cleared) => {
        if (cleared) {
          setOpenModal(null);
          updateServer({ status: null });
        }
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      });
  };

  // plugged.host: a parked server (or one on its way) shows its limbo page on every path
  if (isInLimbo(server.limboState) && !server.isSuspended) {
    return <ServerLimbo />;
  }

  // plugged.host: the setup is answered while the server installs, and shows how applying it goes
  if (!server.isSuspended && !server.isTransferring && !server.nodeMaintenanceEnabled) {
    if (toSetup) {
      return <Navigate to={`/server/${server.uuidShort}${SETUP_PATH}`} replace />;
    }
    if (onSetup && (server.status === null || server.status === 'installing')) {
      return <Outlet />;
    }
  }

  // plugged.host: the Transfer page follows a move while it runs (docs/features/transfers.md)
  if (
    server.isTransferring &&
    !server.nodeMaintenanceEnabled &&
    serverPaths.some((path) => location.pathname.startsWith(`${path}/transfer`))
  ) {
    return <Outlet />;
  }

  if (
    (isConflictingState(server, user) &&
      location.pathname !== `/server/${server.uuid}` &&
      location.pathname !== `/server/${server.uuid}/` &&
      location.pathname !== `/server/${server.uuidShort}` &&
      location.pathname !== `/server/${server.uuidShort}/` &&
      location.pathname !== `/server/${server.uuid}/console/popout` &&
      location.pathname !== `/server/${server.uuidShort}/console/popout`) ||
    server.nodeMaintenanceEnabled ||
    (server.status !== null && serverStatusInfo[server.status].failed)
  ) {
    return (
      <ServerContentContainer title={t('elements.screenBlock.serverConflict.title', {})} hideTitleComponent>
        {acknowledge && (
          <ConfirmationModal
            opened={openModal === 'acknowledgeFailure'}
            onClose={() => setOpenModal(null)}
            title={acknowledge.title}
            onConfirmed={doAcknowledgeFailure}
          >
            {acknowledge.content.md()}
          </ConfirmationModal>
        )}

        <ScreenBlock
          title={t('elements.screenBlock.serverConflict.title', {})}
          content={
            server.isSuspended ? (
              <SuspensionNotice server={server} /> // plugged.host: why, since when, until when
            ) : server.nodeMaintenanceEnabled ? (
              t('elements.screenBlock.serverConflict.contentNodeMaintenance', {})
            ) : server.isTransferring ? (
              t('elements.screenBlock.serverConflict.contentTransferring', {})
            ) : server.status ? (
              serverStatusInfo[server.status].blockContent()
            ) : (
              ''
            )
          }
        >
          {canReadInstallationLogs && server.status === 'install_failed' && (
            <NavLink to={`/admin/servers/${server.uuid}/logs`}>
              <Button variant='outline'>{t('elements.screenBlock.serverConflict.button.viewInstallLogs', {})}</Button>
            </NavLink>
          )}
          {acknowledge && (
            <ServerCan action={acknowledge.permission}>
              <Button color='red' onClick={() => setOpenModal('acknowledgeFailure')}>
                {t('elements.screenBlock.serverConflict.button.acknowledgeFailure', {})}
              </Button>
            </ServerCan>
          )}
          {/* plugged.host: a failed install's server can go, as on the Pelican panel (docs/features/server-settings.md#stuck-installs) */}
          {server.status === 'install_failed' && canDeleteServer && <DeleteServerButton />}
        </ScreenBlock>
      </ServerContentContainer>
    );
  }

  return <Outlet />;
}
