// plugged.host: who is on a FiveM server now, read from FXServer itself, laid out like the Pelican panel's live player
// list (docs/features/fivem.md)
import { faSearch } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { z } from 'zod';
import getFivemPlayers from '@/api/server/fivem/getFivemPlayers.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { fivemPlayersSchema } from '@/lib/schemas/server/fivem.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Live = z.infer<typeof fivemPlayersSchema>;

const base = 'pages.server.fivem.players';
/** FXServer's player list changes as people join; the browser refreshes about as often. */
const REFRESH_SECONDS = 12;

/** Name, server id or part of an identifier, as the Pelican page matches it ("#12" finds id 12). */
export function matchesPlayer(player: Live['players'][number], search: string): boolean {
  const query = search.trim().toLowerCase().replace(/^#/, '');
  if (!query) return true;

  return `${player.name} ${player.id} ${player.identifiers.join(' ')}`.toLowerCase().includes(query);
}

function Empty({ title, description }: { title: string; description: string }) {
  return (
    <div className='theme-fivem-empty' data-shape='tall'>
      <strong>{title}</strong>
      <span>{description}</span>
    </div>
  );
}

function Players({ live }: { live: Live }) {
  const { t } = useTranslations();
  const [search, setSearch] = useState('');

  const rows = useMemo(() => live.players.filter((player) => matchesPlayer(player, search)), [live, search]);
  const count = live.slots
    ? t(`${base}.countSlots`, { count: live.clients, slots: live.slots })
    : t(`${base}.count.${live.clients === 0 ? 'none' : live.clients === 1 ? 'one' : 'other'}`, { count: live.clients });

  return (
    <div className='flex flex-col gap-4'>
      <div className='flex flex-wrap items-center justify-between gap-x-4 gap-y-3'>
        <div className='theme-fivem-state'>
          <span className='theme-fivem-dot' data-online={live.online || undefined} />
          {live.online ? t(`${base}.online`, {}) : t(`${base}.offline`, {})}
          {live.online && <span className='theme-fivem-count'>{count}</span>}
        </div>
        {live.online && live.clients > 0 && (
          <TextInput
            type='search'
            aria-label={t(`${base}.search`, {})}
            placeholder={t(`${base}.search`, {})}
            leftSection={<FontAwesomeIcon icon={faSearch} />}
            value={search}
            onChange={(e) => setSearch(e.currentTarget.value)}
            className='w-full sm:max-w-80'
          />
        )}
      </div>

      {!live.online ? (
        <Empty title={t(`${base}.unreachable`, {})} description={t(`${base}.unreachableDescription`, {})} />
      ) : rows.length === 0 ? (
        search.trim() ? (
          <Empty title={t(`${base}.noMatch`, {})} description={t(`${base}.noMatchDescription`, {})} />
        ) : (
          <Empty title={t(`${base}.nobody`, {})} description={t(`${base}.nobodyDescription`, {})} />
        )
      ) : (
        <div className='flex flex-col gap-2'>
          {rows.map((player) => (
            <div key={player.id} className='theme-fivem-player'>
              <span className='theme-fivem-id'>#{player.id}</span>
              <div className='min-w-0'>
                <div className='theme-fivem-name'>{player.name || t(`${base}.unnamed`, {})}</div>
                {player.identifiers.length > 0 && (
                  <div className='theme-fivem-ids'>
                    {player.identifiers.map((identifier) => (
                      <span key={identifier}>{identifier}</span>
                    ))}
                  </div>
                )}
              </div>
              <span className='theme-fivem-ping' data-slow={player.ping >= 150 || undefined}>
                {t(`${base}.pingValue`, { ping: player.ping })}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default function ServerFivemPlayers() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const query = useQuery({
    queryKey: queryKeys.server(server.uuid).fivem.players(),
    queryFn: () => getFivemPlayers(server.uuid),
    refetchInterval: REFRESH_SECONDS * 1000,
  });

  return (
    <ServerContentContainer
      title={t(`${base}.title`, {})}
      subtitle={t(`${base}.subtitle`, { seconds: REFRESH_SECONDS })}
      contentRight={
        <Button
          variant='default'
          onClick={() => {
            query.refetch();
          }}
          loading={query.isFetching && !query.isLoading}
        >
          {t(`${base}.refresh`, {})}
        </Button>
      }
    >
      <ResourceView
        resource={{ data: query.data, loading: query.isLoading, error: query.error, refetch: () => query.refetch() }}
      >
        {(live) => <Players live={live} />}
      </ResourceView>
    </ServerContentContainer>
  );
}
