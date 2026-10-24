// plugged.host: server.properties and the Bukkit-family YAML as forms (docs/features/minecraft.md)
import { useState } from 'react';
import getGameSettings from '@/api/server/minecraft/getGameSettings.ts';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Tabs from '@/elements/layout/Tabs.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ConfigFileForm from './ConfigFileForm.tsx';

export default function ServerMinecraftSettings() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const [search, setSearch] = useState('');

  const settings = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.settings(),
    queryFn: () => getGameSettings(server.uuid),
  });

  return (
    <ServerContentContainer
      title={t('pages.server.minecraft.settings.title', {})}
      search={search}
      setSearch={setSearch}
    >
      <ResourceView resource={settings}>
        {({ files }) =>
          files.length === 0 ? (
            <Text c='dimmed'>{t('pages.server.minecraft.settings.empty', {})}</Text>
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
