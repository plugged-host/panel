// plugged.host: the server's VS Code page: a devbox with the server's files, as the Pelican panel's
// "VS Code" settings section (plugged-host-devbox's open-in-vscode.blade.php): label/value rows,
// one filled button (docs/features/devboxes.md)
import { useEffect, useState } from 'react';
import { Link } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getDevbox from '@/api/server/devbox/getDevbox.ts';
import openDevboxInBrowser from '@/api/server/devbox/openDevboxInBrowser.ts';
import startDevbox from '@/api/server/devbox/startDevbox.ts';
import stopDevbox from '@/api/server/devbox/stopDevbox.ts';
import updateSharedDevbox from '@/api/server/devbox/updateSharedDevbox.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Group from '@/elements/layout/Group.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { CopyActionIcon, SettingRow, SettingRows } from '@/elements/SettingRows.tsx';
import Text from '@/elements/typography/Text.tsx';
import { tierName } from '@/lib/economy.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverDevboxSchema, serverDevboxSharedRightsSchema } from '@/lib/schemas/server/devbox.ts';
import ServerSettingsShell from '@/pages/server/settings/ServerSettingsShell.tsx';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Devbox = z.infer<typeof serverDevboxSchema>;
type Rights = z.infer<typeof serverDevboxSharedRightsSchema>;

const page = 'pages.server.devbox';
const REMOTE_SSH_URL = 'vscode:extension/ms-vscode-remote.remote-ssh';

function Sharing({ devbox, refresh }: { devbox: Devbox; refresh: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const [saving, setSaving] = useState(false);
  const [confirming, setConfirming] = useState(false);

  const save = (update: { enabled?: boolean; rights?: Rights }) => {
    setSaving(true);
    return updateSharedDevbox(server.uuid, update)
      .then(() => {
        addToast(t(`${page}.toast.saved`, {}), 'success');
      })
      .catch((err) => {
        addToast(httpErrorToHuman(err), 'error');
      })
      .finally(() => {
        setSaving(false);
        setConfirming(false);
        refresh();
      });
  };
  const rights = devbox.sharedRights;
  const plan = tierName('plus');

  return (
    <>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={devbox.shared ? t(`${page}.shared.unshare`, {}) : t(`${page}.shared.share`, {})}
        confirm={devbox.shared ? t(`${page}.shared.unshare`, {}) : t(`${page}.shared.share`, {})}
        onConfirmed={() => save({ enabled: !devbox.shared })}
      >
        <Text size='sm' c='dimmed'>
          {devbox.shared ? t(`${page}.shared.confirmUnshare`, {}) : t(`${page}.shared.confirmShare`, {})}
        </Text>
      </ConfirmationModal>

      <SettingRow
        label={t(`${page}.shared.label`, {})}
        hint={t(`${page}.shared.hint`, { plan })}
        actions={
          <Button
            size='sm'
            variant='default'
            disabled={saving || (!devbox.shared && !devbox.ownerAllowed)}
            onClick={() => setConfirming(true)}
          >
            {devbox.shared ? t(`${page}.shared.unshare`, {}) : t(`${page}.shared.share`, {})}
          </Button>
        }
      >
        {devbox.shared ? t(`${page}.shared.on`, {}) : t(`${page}.shared.off`, {})}
      </SettingRow>

      {/* v3 only: what ph in the shared devbox may do on the server */}
      {devbox.shared && rights && (
        <SettingRow label={t(`${page}.shared.rightsLabel`, {})} hint={t(`${page}.shared.rights`, {})} wrap>
          <Group gap='md'>
            {(['status', 'logs', 'console', 'power'] as const).map((right) => (
              <Checkbox
                key={right}
                label={t(`${page}.shared.right.${right}`, {})}
                checked={rights[right]}
                disabled={saving}
                onChange={(event) => save({ rights: { ...rights, [right]: event.currentTarget.checked } })}
              />
            ))}
          </Group>
        </SettingRow>
      )}
    </>
  );
}

function DevboxRows({ devbox, refresh }: { devbox: Devbox; refresh: () => void }) {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const membershipEnabled = useGlobalStore((state) => !!state.settings.economy?.membershipEnabled);
  const [busy, setBusy] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const plan = tierName('plus');
  const connection = devbox.connection;

  const act = (action: (uuid: string) => Promise<Devbox>) => {
    setBusy(true);
    setError(null);
    action(server.uuid)
      .catch((err) => setError(httpErrorToHuman(err)))
      .finally(() => {
        setBusy(false);
        refresh();
      });
  };

  const openInBrowser = () => {
    setOpening(true);
    setError(null);
    // a window opened before the request, so no popup blocker stops it
    const opened = window.open('', '_blank');
    openDevboxInBrowser(server.uuid)
      .then((url) => {
        if (opened) {
          opened.opener = null;
          opened.location.href = url;
        } else {
          window.location.href = url;
        }
      })
      .catch((err) => {
        opened?.close();
        setError(httpErrorToHuman(err));
      })
      .finally(() => setOpening(false));
  };

  if (devbox.state === 'hidden') {
    return <p className='theme-setting-note'>{t(`${page}.hidden`, {})}</p>;
  }

  if (devbox.state === 'locked') {
    return (
      <SettingRows>
        <SettingRow
          label={t(`${page}.label`, {})}
          hint={t(`${page}.lockedHint`, {})}
          actions={
            membershipEnabled ? (
              <Link to='/account/membership' className='theme-eco-link text-sm'>
                {t(`${page}.getPlan`, { plan })}
              </Link>
            ) : undefined
          }
        >
          {devbox.shared ? t(`${page}.lockedShared`, {}) : t(`${page}.locked`, { plan })}
        </SettingRow>
      </SettingRows>
    );
  }

  const state = devbox.state;
  const hint =
    state === 'ready'
      ? connection
        ? t(`${page}.hint.ready`, { address: `${connection.host}:${connection.port}`, minutes: devbox.idleMinutes })
        : undefined
      : state === 'none' || state === 'stopped'
        ? t(`${page}.hint.what`, { minutes: devbox.idleMinutes })
        : t(`${page}.hint.${state}`, {});
  const stoppable = devbox.canStop && ['ready', 'starting', 'failed', 'mount_missing'].includes(state);

  return (
    <SettingRows>
      <SettingRow
        label={t(`${page}.label`, {})}
        hint={hint}
        actions={
          stoppable ? (
            <Button size='sm' variant='default' loading={busy} onClick={() => act(stopDevbox)}>
              {t(`${page}.button.stop`, {})}
            </Button>
          ) : undefined
        }
      >
        {t(`${page}.state.${state}`, {})}
      </SettingRow>

      {state === 'ready' && connection && (
        <>
          <SettingRow
            label={t(`${page}.code`, {})}
            mono
            hint={t(`${page}.codeHint`, {})}
            actions={<CopyActionIcon content={connection.code ?? ''} label={t(`${page}.copyCode`, {})} />}
          >
            {connection.code}
          </SettingRow>
          <SettingRow
            label={t(`${page}.ssh`, {})}
            mono
            hint={connection.hostKey ? t(`${page}.sshHint`, { key: connection.hostKey }) : undefined}
            actions={<CopyActionIcon content={connection.sshCommand} label={t(`${page}.copySsh`, {})} />}
          >
            {connection.sshCommand}
          </SettingRow>
        </>
      )}

      {devbox.isOwner && (devbox.shared || devbox.ownerAllowed) && <Sharing devbox={devbox} refresh={refresh} />}

      {error && (
        <SettingRow label='' wrap>
          <span className='theme-setting-error'>{error}</span>
        </SettingRow>
      )}

      <div className='theme-setting-open'>
        {state === 'ready' && connection ? (
          <>
            <Button
              onClick={() => {
                window.location.href = connection.vscodeUrl;
              }}
            >
              {t(`${page}.button.vscode`, {})}
            </Button>
            {connection.web && (
              <Button variant='default' loading={opening} onClick={openInBrowser}>
                {t(`${page}.button.browser`, {})}
              </Button>
            )}
          </>
        ) : state === 'none' || state === 'stopped' || state === 'failed' ? (
          <Button loading={busy} onClick={() => act(startDevbox)}>
            {t(`${page}.button.start`, {})}
          </Button>
        ) : null}
        <span className='theme-setting-hint'>
          {t(`${page}.needsExtension`, {})}{' '}
          <a href={REMOTE_SSH_URL} className='theme-eco-link'>
            {t(`${page}.button.install`, {})}
          </a>
        </span>
      </div>
    </SettingRows>
  );
}

export default function ServerDevbox() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).devbox(),
    queryFn: () => getDevbox(server.uuid),
  });

  // creating, starting and stopping take a while: follow them
  useEffect(() => {
    if (!resource.data?.polling) return;

    const interval = setInterval(() => resource.refetch(), 3_000);
    return () => clearInterval(interval);
  }, [resource.data?.polling]);

  return (
    <ServerSettingsShell>
      <TitleCard title={t(`${page}.title`, {})} description={t(`${page}.description`, {})}>
        <ResourceView resource={resource}>
          {(devbox) => <DevboxRows devbox={devbox} refresh={() => resource.invalidate()} />}
        </ResourceView>
      </TitleCard>
    </ServerSettingsShell>
  );
}
