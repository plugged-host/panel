// plugged.host: server.properties and the Bukkit-family YAML as forms (docs/features/minecraft.md)
import { faRotateRight } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import getGameSettings from '@/api/server/minecraft/getGameSettings.ts';
import sendPowerAction from '@/api/server/sendPowerAction.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Tabs from '@/elements/layout/Tabs.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ConfigFileForm from './ConfigFileForm.tsx';

const base = 'pages.server.minecraft.settings';

export default function ServerMinecraftSettings() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const canRestart = useServerCan('control.restart');
  const [search, setSearch] = useState('');
  const [restarting, setRestarting] = useState(false);

  const settings = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.settings(),
    queryFn: () => getGameSettings(server.uuid),
  });

  // over the API like the console's power buttons: a start gate holding the start answers with its page
  const restart = () =>
    sendPowerAction(server.uuid, 'restart').then(({ url }) => {
      setRestarting(false);
      if (url?.startsWith('/')) {
        navigate(url);
      } else if (url) {
        window.location.assign(url);
      } else {
        addToast(t(`${base}.toast.restarting`, {}), 'success');
      }
    });

  return (
    <ServerContentContainer
      title={t(`${base}.title`, {})}
      search={search}
      setSearch={setSearch}
      contentRight={
        // a power control: icon-only like the console's, so Save stays the page's one filled button
        canRestart ? (
          <Tooltip label={t(`${base}.button.restart`, {})}>
            <ActionIcon
              size='input-sm'
              variant='subtle'
              color='gray'
              aria-label={t(`${base}.button.restart`, {})}
              onClick={() => setRestarting(true)}
            >
              <FontAwesomeIcon icon={faRotateRight} />
            </ActionIcon>
          </Tooltip>
        ) : undefined
      }
    >
      <ConfirmationModal
        opened={restarting}
        onClose={() => setRestarting(false)}
        title={t(`${base}.restart.title`, {})}
        confirm={t(`${base}.button.restart`, {})}
        confirmColor='blue'
        onConfirmed={restart}
      >
        {t(`${base}.restart.description`, {})}
      </ConfirmationModal>

      <ResourceView resource={settings}>
        {({ files }) =>
          files.length === 0 ? (
            <Text c='dimmed'>{t(`${base}.empty`, {})}</Text>
          ) : (
            <Tabs defaultValue={files[0].id}>
              <Tabs.List>
                {files.map((file) => (
                  <Tabs.Tab key={file.id} value={file.id}>
                    {file.path.split('/').pop()}
                  </Tabs.Tab>
                ))}
              </Tabs.List>

              {files.map((file) => (
                <Tabs.Panel key={file.id} value={file.id} pt='md'>
                  <ConfigFileForm file={file} search={search} onSaved={settings.refetch} />
                </Tabs.Panel>
              ))}
            </Tabs>
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
