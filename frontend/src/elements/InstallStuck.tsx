// plugged.host: the way out of an install that runs far too long, as the Pelican panel's installing page offered after 30 minutes (docs/features/server-settings.md#stuck-installs)
import { useEffect, useState } from 'react';
import retryServerInstall from '@/api/server/settings/retryServerInstall.ts';
import Button from '@/elements/buttons/Button.tsx';
import Group from '@/elements/layout/Group.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Text from '@/elements/typography/Text.tsx';
import DeleteServerModal from '@/pages/server/settings/modals/DeleteServerModal.tsx';
import { useCanDeleteServer } from '@/pages/server/settings/useSettingsSections.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore, useServerStoreApi } from '@/stores/server.ts';

/** Minutes after which an install counts as stuck (shared/src/stuck_installs.rs). */
const STUCK_AFTER_MINUTES = 30;

/** Whether the server's install has run for longer than it should, rechecked every 30 seconds. */
function useInstallStuck(): boolean {
  const serverStoreApi = useServerStoreApi();
  const since = useServerStore((state) =>
    state.server.status === 'installing' ? (state.server.installingSince?.getTime() ?? null) : null,
  );
  const timeOffset = useGlobalStore((state) => state.timeOffset);
  const [now, setNow] = useState(() => Date.now());

  // an install that began after the page loaded has no start from the panel: it began now; once it
  // ends (not when another page opens), the next one starts its own clock
  useEffect(() => {
    const { server, updateServer } = serverStoreApi.getState();
    if (server.status === 'installing' && !server.installingSince) {
      updateServer({ installingSince: new Date(Date.now() - useGlobalStore.getState().timeOffset) });
    }

    return () => {
      const { server, updateServer } = serverStoreApi.getState();
      if (server.status !== 'installing') {
        updateServer({ installingSince: null });
      }
    };
  }, [serverStoreApi]);

  useEffect(() => {
    if (since === null) return;

    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, [since]);

  return since !== null && now - timeOffset - since >= STUCK_AFTER_MINUTES * 60_000;
}

export default function InstallStuck() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const stuck = useInstallStuck();
  const canReinstall = useServerCan('settings.install');
  const canDelete = useCanDeleteServer();
  const [opened, setOpened] = useState(false);

  if (!stuck || (!canReinstall && !canDelete)) {
    return null;
  }

  const doReinstall = async () => {
    await retryServerInstall(server.uuid);

    addToast(t('pages.server.console.installStuck.toast.reinstalling', {}), 'success');
    updateServer({ installingSince: new Date(Date.now() - useGlobalStore.getState().timeOffset) });
    setOpened(false);
  };

  return (
    <div className='mt-2'>
      <ConfirmationModal
        opened={opened}
        onClose={() => setOpened(false)}
        title={t('pages.server.console.installStuck.reinstall.title', {})}
        confirm={t('pages.server.console.installStuck.reinstall.button', {})}
        confirmColor='blue'
        onConfirmed={doReinstall}
      >
        {t('pages.server.console.installStuck.reinstall.content', {})}
      </ConfirmationModal>

      <Text size='sm' mb='xs'>
        {t('pages.server.console.installStuck.content', {})}
      </Text>
      <Group>
        {canReinstall && (
          <Button variant='default' onClick={() => setOpened(true)}>
            {t('pages.server.console.installStuck.reinstall.button', {})}
          </Button>
        )}
        <DeleteServerButton />
      </Group>
    </div>
  );
}

/** Delete Server with its confirmation, for a server whose install is stuck or failed (as the
 * Pelican panel's installing page offered); nothing without the right to delete it. */
export function DeleteServerButton() {
  const { t } = useTranslations();
  const canDelete = useCanDeleteServer();
  const [opened, setOpened] = useState(false);

  if (!canDelete) {
    return null;
  }

  return (
    <>
      <DeleteServerModal opened={opened} onClose={() => setOpened(false)} />
      <Button variant='default' onClick={() => setOpened(true)}>
        {t('pages.server.settings.delete.button', {})}
      </Button>
    </>
  );
}
