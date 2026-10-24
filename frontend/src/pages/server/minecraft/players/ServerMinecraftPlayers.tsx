// plugged.host: the whitelist, operators and bans of a Minecraft server (docs/features/minecraft.md)
import { useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getPlayers from '@/api/server/minecraft/getPlayers.ts';
import setWhitelistEnforced from '@/api/server/minecraft/setWhitelistEnforced.ts';
import Button from '@/elements/buttons/Button.tsx';
import { ServerCan } from '@/elements/Can.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import Switch from '@/elements/input/Switch.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftRosterSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import AddPlayerModal from './AddPlayerModal.tsx';
import PlayerRow from './PlayerRow.tsx';

function Players({ roster, search }: { roster: z.infer<typeof minecraftRosterSchema>; search: string }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.players');
  const [saving, setSaving] = useState(false);
  const base = 'pages.server.minecraft.players';

  const rows = useMemo(() => {
    const query = search.trim().toLowerCase();
    return query ? roster.players.filter((player) => player.name.toLowerCase().includes(query)) : roster.players;
  }, [roster, search]);

  const enforce = (enabled: boolean) => {
    setSaving(true);

    setWhitelistEnforced(server.uuid, enabled)
      .then((live) => {
        const message = enabled ? t(`${base}.toast.enforcedOn`, {}) : t(`${base}.toast.enforcedOff`, {});
        addToast(live ? message : `${message} ${t(`${base}.toast.atNextStart`, {})}`, 'success');
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => {
        setSaving(false);
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.players() });
      });
  };

  return (
    <Stack gap='md'>
      <Switch
        label={t(`${base}.enforced`, {})}
        description={t(`${base}.enforcedDescription`, {})}
        checked={roster.enforced}
        disabled={!canManage || saving}
        onChange={(e) => enforce(e.currentTarget.checked)}
      />

      {!roster.onlineMode && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.offlineMode`, {})}
        </Text>
      )}
      {!roster.running && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.stopped`, {})}
        </Text>
      )}

      <Table
        columns={[t(`${base}.table.player`, {}), t(`${base}.table.access`, {}), '']}
        pagination={{ total: rows.length, perPage: Math.max(rows.length, 1), page: 1, data: rows }}
        empty={
          <Text size='sm' c='dimmed'>
            {search.trim() ? t(`${base}.noMatch`, {}) : t(`${base}.empty`, {})}
          </Text>
        }
      >
        {rows.map((player) => (
          <PlayerRow key={player.name} player={player} />
        ))}
      </Table>
    </Stack>
  );
}

export default function ServerMinecraftPlayers() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const [search, setSearch] = useState('');
  const [adding, setAdding] = useState(false);

  const roster = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.players(),
    queryFn: () => getPlayers(server.uuid),
  });

  return (
    <ServerContentContainer
      title={t('pages.server.minecraft.players.title', {})}
      search={search}
      setSearch={setSearch}
      contentRight={
        <ServerCan action='minecraft.players'>
          <Button onClick={() => setAdding(true)}>{t('pages.server.minecraft.players.button.add', {})}</Button>
        </ServerCan>
      }
    >
      <AddPlayerModal opened={adding} onClose={() => setAdding(false)} onlineMode={roster.data?.onlineMode ?? true} />

      <ResourceView resource={roster}>{(roster) => <Players roster={roster} search={search} />}</ResourceView>
    </ServerContentContainer>
  );
}
