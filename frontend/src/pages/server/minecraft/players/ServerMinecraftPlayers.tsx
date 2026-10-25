// plugged.host: the whitelist, operators and bans of a Minecraft server, laid out like the Pelican
// panel's Players page: the Whitelist section, then one table of everyone on a list or who has
// played, each opening the player's page (docs/features/minecraft.md)
import { faArrowsRotate, faSearch, faUserSlash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useId, useMemo, useState } from 'react';
import { z } from 'zod';
import { getHttpStatus, httpErrorToHuman } from '@/api/axios.ts';
import getOnlinePlayers from '@/api/server/minecraft/getOnlinePlayers.ts';
import getPlayers from '@/api/server/minecraft/getPlayers.ts';
import setWhitelistEnforced from '@/api/server/minecraft/setWhitelistEnforced.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftRosterSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import AddPlayerModal from './AddPlayerModal.tsx';
import PlayerRow from './PlayerRow.tsx';

const base = 'pages.server.minecraft.players';

function Players({ roster }: { roster: z.infer<typeof minecraftRosterSchema> }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.players');
  const canConsole = useServerCan('control.console');
  const switchId = useId();
  const [saving, setSaving] = useState(false);
  const [adding, setAdding] = useState(false);
  const [search, setSearch] = useState('');

  const rows = useMemo(() => {
    const query = search.trim().toLowerCase();
    return query
      ? roster.players.filter(
          (player) => player.name.toLowerCase().includes(query) || player.uuid.toLowerCase().includes(query),
        )
      : roster.players;
  }, [roster, search]);

  // who is on: asked once (each ask types `list` into the console), never on a timer
  const online = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.online(),
    queryFn: () => getOnlinePlayers(server.uuid),
    enabled: roster.running && canConsole,
    silent: true,
  });
  const isOnline = (player: z.infer<typeof minecraftRosterSchema>['players'][number]) =>
    !!online.data?.players?.some((entry) =>
      entry.uuid && player.uuid
        ? entry.uuid.toLowerCase() === player.uuid.toLowerCase()
        : !!player.name && entry.name.toLowerCase() === player.name.toLowerCase(),
    );

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
    <div className='flex flex-col gap-6'>
      <AddPlayerModal opened={adding} onClose={() => setAdding(false)} onlineMode={roster.onlineMode} />

      <TitleCard title={t(`${base}.section.title`, {})} description={t(`${base}.section.description`, {})}>
        <div className='theme-stack-field'>
          <label htmlFor={switchId} className='theme-stack-field-label'>
            {t(`${base}.enforced`, {})}
          </label>
          <Switch
            id={switchId}
            checked={roster.enforced}
            disabled={!canManage || saving}
            onChange={(e) => enforce(e.currentTarget.checked)}
          />
          {!roster.onlineMode && <p className='theme-stack-field-hint'>{t(`${base}.offlineMode`, {})}</p>}
        </div>
      </TitleCard>

      {/* Filament's table: the header with its action, the toolbar with the search, the rows. */}
      <div>
        {canManage && (
          <div className='theme-table-header'>
            <Button onClick={() => setAdding(true)}>{t(`${base}.button.add`, {})}</Button>
          </div>
        )}
        <div className='theme-table-toolbar'>
          <TextInput
            className='theme-header-search'
            placeholder={t(`${base}.search`, {})}
            aria-label={t(`${base}.search`, {})}
            leftSection={<FontAwesomeIcon icon={faSearch} />}
            value={search}
            onChange={(e) => setSearch(e.currentTarget.value)}
          />
        </div>

        <Table
          columns={[
            t(`${base}.table.player`, {}),
            t(`${base}.table.whitelisted`, {}),
            t(`${base}.table.operator`, {}),
            t(`${base}.table.banned`, {}),
            ...(canManage ? [''] : []),
          ]}
          pagination={{ total: rows.length, perPage: Math.max(rows.length, 1), page: 1, data: rows }}
          empty={
            <EmptyState
              flush
              icon={faUserSlash}
              title={search.trim() ? t(`${base}.noMatch`, {}) : t(`${base}.emptyHeading`, {})}
              description={search.trim() ? null : t(`${base}.emptyBody`, {})}
            />
          }
        >
          {rows.map((player) => (
            <PlayerRow
              key={player.uuid || player.name}
              player={player}
              canManage={canManage}
              online={isOnline(player)}
            />
          ))}
        </Table>
      </div>
    </div>
  );
}

export default function ServerMinecraftPlayers() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const [reloading, setReloading] = useState(false);

  // Silent: when the lists cannot be read the page says so itself, like the Pelican panel does.
  const roster = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.players(),
    queryFn: () => getPlayers(server.uuid),
    silent: true,
  });

  const reload = () => {
    setReloading(true);
    roster
      .refetch()
      .then(({ error }) => {
        if (error && roster.data) addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setReloading(false));
  };

  const unreachable = !roster.data && !!roster.error && getHttpStatus(roster.error) !== 404;

  return (
    <ServerContentContainer
      title={t(`${base}.title`, {})}
      subtitle={t(`${base}.subtitle`, {})}
      contentRight={
        <Tooltip label={t(`${base}.button.refresh`, {})}>
          <ActionIcon size='input-sm' variant='subtle' color='gray' loading={reloading} onClick={reload}>
            <FontAwesomeIcon icon={faArrowsRotate} />
          </ActionIcon>
        </Tooltip>
      }
    >
      {unreachable && !roster.loading ? (
        <TitleCard title={t(`${base}.unreachableHeading`, {})} description={t(`${base}.unreachable`, {})} />
      ) : (
        <ResourceView resource={roster}>{(data) => <Players roster={data} />}</ResourceView>
      )}
    </ServerContentContainer>
  );
}
