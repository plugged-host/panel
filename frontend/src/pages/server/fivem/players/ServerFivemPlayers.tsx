// plugged.host: who is on a FiveM server now, read from FXServer itself (docs/features/fivem.md)
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { z } from 'zod';
import getFivemPlayers from '@/api/server/fivem/getFivemPlayers.ts';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { fivemPlayersSchema } from '@/lib/schemas/server/fivem.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

const base = 'pages.server.fivem.players';
/** FXServer's player list changes as people join; the browser refreshes about as often. */
const REFRESH_SECONDS = 12;

function Players({ live, search }: { live: z.infer<typeof fivemPlayersSchema>; search: string }) {
  const { t } = useTranslations();

  const rows = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return live.players;

    return live.players.filter(
      (player) =>
        player.name.toLowerCase().includes(query) ||
        String(player.id) === query ||
        player.identifiers.some((identifier) => identifier.toLowerCase().includes(query)),
    );
  }, [live, search]);

  if (!live.online) {
    return (
      <Stack gap={2}>
        <Text fw={500}>{t(`${base}.unreachable`, {})}</Text>
        <Text size='sm' c='dimmed'>
          {t(`${base}.unreachableDescription`, {})}
        </Text>
      </Stack>
    );
  }

  return (
    <Stack gap='md'>
      <Text size='sm' c='dimmed'>
        {live.slots !== null
          ? t(`${base}.countSlots`, { count: live.clients, slots: live.slots })
          : t(`${base}.count`, { count: live.clients })}
        {live.hostname ? ` · ${live.hostname}` : ''}
      </Text>

      <Table
        columns={[t(`${base}.table.id`, {}), t(`${base}.table.player`, {}), t(`${base}.table.ping`, {})]}
        pagination={{ total: rows.length, perPage: Math.max(rows.length, 1), page: 1, data: rows }}
        empty={
          <Text size='sm' c='dimmed'>
            {search.trim() ? t(`${base}.noMatch`, {}) : t(`${base}.nobody`, {})}
          </Text>
        }
      >
        {rows.map((player) => (
          <TableRow key={player.id}>
            <TableData>{player.id}</TableData>
            <TableData>
              <Stack gap={2}>
                <Text size='sm'>{player.name || t(`${base}.unnamed`, {})}</Text>
                {player.identifiers.length > 0 && (
                  <Text size='xs' c='dimmed'>
                    {player.identifiers.map((identifier) => (
                      <Code key={identifier} className='me-1'>
                        {identifier}
                      </Code>
                    ))}
                  </Text>
                )}
              </Stack>
            </TableData>
            <TableData>{t(`${base}.pingValue`, { ping: player.ping })}</TableData>
          </TableRow>
        ))}
      </Table>
    </Stack>
  );
}

export default function ServerFivemPlayers() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const [search, setSearch] = useState('');

  const query = useQuery({
    queryKey: queryKeys.server(server.uuid).fivem.players(),
    queryFn: () => getFivemPlayers(server.uuid),
    refetchInterval: REFRESH_SECONDS * 1000,
  });

  return (
    <ServerContentContainer title={t(`${base}.title`, {})} search={search} setSearch={setSearch}>
      <Text size='sm' c='dimmed' mb='md'>
        {t(`${base}.description`, { seconds: REFRESH_SECONDS })}
      </Text>
      <ResourceView
        resource={{ data: query.data, loading: query.isLoading, error: query.error, refetch: () => query.refetch() }}
      >
        {(live) => <Players live={live} search={search} />}
      </ResourceView>
    </ServerContentContainer>
  );
}
