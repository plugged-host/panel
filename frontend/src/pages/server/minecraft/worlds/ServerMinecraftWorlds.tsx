// plugged.host: a Minecraft server's worlds (docs/features/minecraft.md)
import { useEffect, useMemo, useRef, useState } from 'react';
import { z } from 'zod';
import getWorlds from '@/api/server/minecraft/getWorlds.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftWorldsSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import NewWorldModal from './NewWorldModal.tsx';
import UploadWorldModal from './UploadWorldModal.tsx';
import WorldRow from './WorldRow.tsx';

function Worlds({ worlds, search }: { worlds: z.infer<typeof minecraftWorldsSchema>; search: string }) {
  const { t } = useTranslations();
  const base = 'pages.server.minecraft.worlds';

  const rows = useMemo(() => {
    const query = search.trim().toLowerCase();
    return query ? worlds.worlds.filter((world) => world.name.toLowerCase().includes(query)) : worlds.worlds;
  }, [worlds, search]);

  return (
    <Stack gap='md'>
      {worlds.import?.state === 'running' && (
        <Alert color='blue'>{t(`${base}.importing`, { file: worlds.import.archive })}</Alert>
      )}
      {worlds.import?.state === 'failed' && (
        <Alert color='red'>
          {t(`${base}.importFailed`, { file: worlds.import.archive, error: worlds.import.error ?? '' })}
        </Alert>
      )}
      {worlds.running && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.running`, {})}
        </Text>
      )}

      <Table
        columns={[t(`${base}.table.world`, {}), t(`${base}.table.modified`, {}), '']}
        pagination={{ total: rows.length, perPage: Math.max(rows.length, 1), page: 1, data: rows }}
        empty={
          <Text size='sm' c='dimmed'>
            {t(`${base}.noMatch`, {})}
          </Text>
        }
      >
        {rows.map((world) => (
          <WorldRow key={world.name} world={world} running={worlds.running} trash={worlds.trash} />
        ))}
      </Table>
    </Stack>
  );
}

export default function ServerMinecraftWorlds() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.worlds');
  const canUpload = useServerCan(['minecraft.worlds', 'files.create'], false);
  const [search, setSearch] = useState('');
  const [modal, setModal] = useState<'new' | 'upload' | null>(null);
  const base = 'pages.server.minecraft.worlds';

  const worlds = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.worlds(),
    queryFn: () => getWorlds(server.uuid),
  });

  // An import runs after its request: poll until it is done, then say how it ended.
  const importState = worlds.data?.import?.state;
  const previousState = useRef(importState);
  useEffect(() => {
    if (previousState.current === 'running' && importState === 'done') {
      const kept = worlds.data?.import?.kept;
      addToast(
        kept
          ? `${t(`${base}.toast.imported`, {})} ${t(`${base}.toast.kept`, { name: kept })}`
          : t(`${base}.toast.imported`, {}),
        'success',
      );
    }
    previousState.current = importState;

    if (importState !== 'running') return;
    const timer = setInterval(() => worlds.refetch(), 3000);
    return () => clearInterval(timer);
  }, [importState]);

  const busy = !worlds.data || worlds.data.running || importState === 'running';

  return (
    <ServerContentContainer
      title={t(`${base}.title`, {})}
      search={search}
      setSearch={setSearch}
      contentRight={
        <Group gap='xs'>
          {canUpload && (
            <Button variant='default' disabled={busy} onClick={() => setModal('upload')}>
              {t(`${base}.button.upload`, {})}
            </Button>
          )}
          {canManage && (
            <Button disabled={busy} onClick={() => setModal('new')}>
              {t(`${base}.button.new`, {})}
            </Button>
          )}
        </Group>
      }
    >
      {worlds.data && <NewWorldModal opened={modal === 'new'} onClose={() => setModal(null)} worlds={worlds.data} />}
      <UploadWorldModal opened={modal === 'upload'} onClose={() => setModal(null)} />

      <ResourceView resource={worlds}>{(worlds) => <Worlds worlds={worlds} search={search} />}</ResourceView>
    </ServerContentContainer>
  );
}
