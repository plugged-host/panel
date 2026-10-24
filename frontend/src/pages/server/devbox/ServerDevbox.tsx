// plugged.host: the server's VS Code page: a devbox with the server's files (docs/features/devboxes.md)
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getDevbox from '@/api/server/devbox/getDevbox.ts';
import openDevboxInBrowser from '@/api/server/devbox/openDevboxInBrowser.ts';
import startDevbox from '@/api/server/devbox/startDevbox.ts';
import stopDevbox from '@/api/server/devbox/stopDevbox.ts';
import updateSharedDevbox from '@/api/server/devbox/updateSharedDevbox.ts';
import Button from '@/elements/buttons/Button.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Anchor from '@/elements/typography/Anchor.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverDevboxSchema, serverDevboxSharedRightsSchema } from '@/lib/schemas/server/devbox.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Devbox = z.infer<typeof serverDevboxSchema>;
type Rights = z.infer<typeof serverDevboxSharedRightsSchema>;

const page = 'pages.server.devbox';

function Field({ label, value, copy = true }: { label: string; value: string; copy?: boolean }) {
  const input = <TextInput label={label} value={value} className='pointer-events-none' readOnly />;

  return copy ? (
    <CopyOnClick content={value} className='text-start'>
      {input}
    </CopyOnClick>
  ) : (
    input
  );
}

function Connection({ devbox, busy, onStop }: { devbox: Devbox; busy: boolean; onStop: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const [opening, setOpening] = useState(false);
  const connection = devbox.connection!;

  const openInBrowser = () => {
    setOpening(true);
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
        addToast(httpErrorToHuman(err), 'error');
      })
      .finally(() => setOpening(false));
  };

  return (
    <Stack gap='lg'>
      <Stack gap={4}>
        <Text>{t(`${page}.ready`, {})}</Text>
        <Text size='sm' c='dimmed'>
          {connection.connections > 0
            ? t(`${page}.connections`, { count: connection.connections })
            : t(`${page}.idle`, { minutes: devbox.idleMinutes })}
        </Text>
      </Stack>

      <Group>
        <Anchor href={connection.vscodeUrl}>
          <Button>{t(`${page}.button.vscode`, {})}</Button>
        </Anchor>
        {connection.web && (
          <Button variant='default' loading={opening} onClick={openInBrowser}>
            {t(`${page}.button.browser`, {})}
          </Button>
        )}
      </Group>

      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        <Field label={t('common.form.host', {})} value={connection.host} />
        <Field label={t(`${page}.port`, {})} value={String(connection.port)} />
        <Field label={t('common.form.username', {})} value={connection.username} />
        <Field label={t(`${page}.code`, {})} value={connection.code ?? ''} />
      </div>
      <Stack gap={4}>
        <Field label={t(`${page}.sshCommand`, {})} value={connection.sshCommand} />
        <Text size='xs' c='dimmed'>
          {t(`${page}.codeHelp`, {})}
        </Text>
      </Stack>
      {connection.hostKey && <Field label={t(`${page}.hostKey`, {})} value={connection.hostKey} />}

      {devbox.canStop && (
        <Group>
          <Button variant='default' loading={busy} onClick={onStop}>
            {t(`${page}.button.stop`, {})}
          </Button>
        </Group>
      )}
    </Stack>
  );
}

function Sharing({ devbox, refresh }: { devbox: Devbox; refresh: (devbox: Devbox) => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const [saving, setSaving] = useState(false);

  const save = (update: { enabled?: boolean; rights?: Rights }) => {
    setSaving(true);
    updateSharedDevbox(server.uuid, update)
      .then((next) => {
        addToast(t(`${page}.toast.saved`, {}), 'success');
        refresh(next);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };
  const rights = devbox.sharedRights;

  return (
    <Stack gap='sm'>
      <Title order={3} fz='md' fw={600}>
        {t(`${page}.shared.title`, {})}
      </Title>
      <Switch
        label={t(`${page}.shared.enabled`, {})}
        description={t(`${page}.shared.enabledDescription`, {})}
        checked={devbox.shared}
        disabled={saving || (!devbox.shared && !devbox.ownerAllowed)}
        onChange={(event) => save({ enabled: event.currentTarget.checked })}
      />
      {devbox.shared && rights && (
        <Stack gap='xs'>
          <Text size='sm' c='dimmed'>
            {t(`${page}.shared.rights`, {})}
          </Text>
          {(['status', 'logs', 'console', 'power'] as const).map((right) => (
            <Checkbox
              key={right}
              label={t(`${page}.shared.right.${right}`, {})}
              checked={rights[right]}
              disabled={saving}
              onChange={(event) => save({ rights: { ...rights, [right]: event.currentTarget.checked } })}
            />
          ))}
        </Stack>
      )}
    </Stack>
  );
}

export default function ServerDevbox() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const membershipEnabled = useGlobalStore((state) => !!state.settings.economy?.membershipEnabled);
  const [busy, setBusy] = useState(false);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).devbox(),
    queryFn: () => getDevbox(server.uuid),
  });

  // starts and stops take a while: follow them
  useEffect(() => {
    if (!resource.data?.polling) return;

    const interval = setInterval(() => resource.refetch(), 3_000);
    return () => clearInterval(interval);
  }, [resource.data?.polling]);

  const act = (action: (uuid: string) => Promise<Devbox>, toast: 'starting' | 'stopping') => {
    setBusy(true);
    action(server.uuid)
      .then(() => addToast(t(`${page}.toast.${toast}`, {}), 'success'))
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => {
        setBusy(false);
        resource.invalidate();
      });
  };
  const start = () => act(startDevbox, 'starting');
  const stop = () => act(stopDevbox, 'stopping');

  return (
    <ServerContentContainer title={t(`${page}.title`, {})}>
      <ResourceView resource={resource}>
        {(devbox) => (
          <Stack gap='xl'>
            {devbox.state === 'hidden' ? (
              <Text c='dimmed'>{t(`${page}.hidden`, {})}</Text>
            ) : devbox.state === 'locked' ? (
              <Stack gap='xs' align='flex-start'>
                <Text>{devbox.shared ? t(`${page}.lockedShared`, {}) : t(`${page}.locked`, {})}</Text>
                {membershipEnabled && (
                  <Button onClick={() => navigate('/account/membership')}>{t(`${page}.button.membership`, {})}</Button>
                )}
              </Stack>
            ) : devbox.state === 'none' || devbox.state === 'stopped' ? (
              <Stack gap='xs' align='flex-start'>
                <Text>{t(`${page}.about`, {})}</Text>
                <Text size='sm' c='dimmed'>
                  {devbox.kind === 'shared' ? t(`${page}.aboutShared`, {}) : t(`${page}.aboutPersonal`, {})}{' '}
                  {t(`${page}.idle`, { minutes: devbox.idleMinutes })}
                </Text>
                <Button mt='xs' loading={busy} onClick={start}>
                  {t(`${page}.button.start`, {})}
                </Button>
              </Stack>
            ) : devbox.state === 'starting' || devbox.state === 'stopping' ? (
              <Group gap='sm'>
                <Spinner size={20} />
                <Text>{devbox.state === 'starting' ? t(`${page}.starting`, {}) : t(`${page}.stopping`, {})}</Text>
              </Group>
            ) : devbox.state === 'failed' || devbox.state === 'mount_missing' ? (
              <Stack gap='xs' align='flex-start'>
                <Text c='red'>
                  {devbox.state === 'failed' ? t(`${page}.failed`, {}) : t(`${page}.mountMissing`, {})}
                </Text>
                {devbox.canStop && (
                  <Button variant='default' loading={busy} onClick={stop}>
                    {t(`${page}.button.stop`, {})}
                  </Button>
                )}
              </Stack>
            ) : (
              <Connection devbox={devbox} busy={busy} onStop={stop} />
            )}

            {devbox.isOwner && devbox.state !== 'hidden' && (devbox.shared || devbox.ownerAllowed) && (
              <Sharing devbox={devbox} refresh={() => resource.invalidate()} />
            )}
          </Stack>
        )}
      </ResourceView>
    </ServerContentContainer>
  );
}
