// plugged.host: the plugins or mods installed on a Minecraft server (docs/features/minecraft.md)
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import getInstalledContent from '@/api/server/minecraft/getInstalledContent.ts';
import Table from '@/elements/data-display/Table.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ContentPage, { ContentOptions } from './ContentPage.tsx';
import InstalledRow from './InstalledRow.tsx';

// the list identifies a few jars per request; ask again until it is done
const IDENTIFY_AGAIN_MS = 1500;

function InstalledList({ options, search }: { options: ContentOptions; search: string }) {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);

  const installed = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.installed(),
    queryFn: () => getInstalledContent(server.uuid),
    keepPrevious: true,
  });
  const pending = installed.data?.pending ?? 0;

  useEffect(() => {
    if (!pending) return;

    const timeout = setTimeout(() => installed.refetch(), IDENTIFY_AGAIN_MS);
    return () => clearTimeout(timeout);
  }, [installed.data]);

  const rows = useMemo(() => {
    const query = search.trim().toLowerCase();
    const jars = installed.data?.installed ?? [];

    return query
      ? jars.filter((jar) => jar.name.toLowerCase().includes(query) || jar.file.toLowerCase().includes(query))
      : jars;
  }, [installed.data, search]);

  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.installed() });

  return (
    <Stack gap='sm'>
      {pending > 0 && (
        <Text size='sm' c='dimmed'>
          {t('pages.server.minecraft.content.installed.identifying', { count: pending })}
        </Text>
      )}

      <Table
        columns={[
          t('common.table.columns.name', {}),
          t('common.table.columns.source', {}),
          t('common.table.columns.version', {}),
          t('common.table.columns.size', {}),
          '',
        ]}
        loading={installed.loading && !installed.data}
        pagination={{ total: rows.length, perPage: Math.max(rows.length, 1), page: 1, data: rows }}
        empty={
          <Text size='sm' c='dimmed'>
            {search.trim()
              ? t('pages.server.minecraft.content.installed.noMatch', {})
              : t('pages.server.minecraft.content.installed.empty', { directory: options.directory ?? '' })}
          </Text>
        }
      >
        {rows.map((jar) => (
          <InstalledRow key={jar.file} jar={jar} directory={options.directory ?? ''} onChanged={refresh} />
        ))}
      </Table>
    </Stack>
  );
}

export default function ServerMinecraftContent() {
  const [search, setSearch] = useState('');

  return (
    <ContentPage search={search} setSearch={setSearch}>
      {(options) => <InstalledList options={options} search={search} />}
    </ContentPage>
  );
}
