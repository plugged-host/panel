// plugged.host: finding plugins and mods on Modrinth, CurseForge and SpigotMC (docs/features/minecraft.md)
import { useDebouncedValue } from '@mantine/hooks';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { z } from 'zod';
import getInstalledContent from '@/api/server/minecraft/getInstalledContent.ts';
import searchContent, { ContentSearchParams } from '@/api/server/minecraft/searchContent.ts';
import Table from '@/elements/data-display/Table.tsx';
import Select from '@/elements/input/Select.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftKindSchema, minecraftSourceSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ContentPage, { ContentOptions } from './ContentPage.tsx';
import { minecraftSourceLabels } from './labels.ts';
import ProjectRow from './ProjectRow.tsx';

type Source = z.infer<typeof minecraftSourceSchema>;
type Kind = z.infer<typeof minecraftKindSchema>;

function Browse({ options, query }: { options: ContentOptions; query: string }) {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.content.browse';

  // modpacks have their own tab: they bring a loader and a version
  const kinds = options.kinds.filter((kind) => kind !== 'modpack');

  const [source, setSource] = useState<Source>(options.sources[0] ?? 'modrinth');
  const [kind, setKind] = useState<Kind | null>(null);
  const [anyVersion, setAnyVersion] = useState(false);
  const [page, setPage] = useState(1);

  useEffect(() => setPage(1), [source, kind, anyVersion, query]);

  const params: ContentSearchParams = {
    source,
    kind: kind ?? kinds[0] ?? null,
    query,
    gameVersion: anyVersion ? '' : null,
    page,
  };

  const results = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.search(params),
    queryFn: () => searchContent(server.uuid, params),
    keepPrevious: true,
  });

  const installed = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.installed(),
    queryFn: () => getInstalledContent(server.uuid),
    silent: true,
  });

  const installedIds = useMemo(
    () =>
      new Set(
        (installed.data?.installed ?? [])
          .filter((jar) => jar.source && jar.projectId)
          .map((jar) => `${jar.source}:${jar.projectId}`),
      ),
    [installed.data],
  );

  const projects = results.data?.projects;
  const gameVersion = results.data?.gameVersion ?? options.gameVersion;

  return (
    <Stack gap='sm'>
      <Group gap='sm' align='flex-end'>
        {options.sources.length > 1 && (
          <Select
            label={t(`${base}.filter.source`, {})}
            data={options.sources.map((source) => ({ value: source, label: minecraftSourceLabels[source] }))}
            value={source}
            onChange={(value) => value && setSource(value as Source)}
          />
        )}
        {kinds.length > 1 && (
          <Select
            label={t(`${base}.filter.kind`, {})}
            data={kinds.map((kind) => ({ value: kind, label: t(`${base}.kind.${kind}`, {}) }))}
            value={params.kind}
            onChange={(value) => value && setKind(value as Kind)}
          />
        )}
        <Select
          label={t(`${base}.filter.version`, {})}
          data={[
            {
              value: 'server',
              label: options.gameVersion
                ? t(`${base}.version.server`, { version: options.gameVersion })
                : t(`${base}.version.serverUnknown`, {}),
            },
            { value: 'any', label: t(`${base}.version.any`, {}) },
          ]}
          value={anyVersion ? 'any' : 'server'}
          onChange={(value) => setAnyVersion(value === 'any')}
        />
      </Group>

      <Table
        columns={[
          t(`${base}.table.project`, {}),
          t('common.table.columns.author', {}),
          t(`${base}.table.downloads`, {}),
          '',
          '',
        ]}
        loading={results.loading}
        pagination={projects ? { ...projects, data: projects.hits } : undefined}
        onPageSelect={setPage}
        empty={
          <Text size='sm' c='dimmed'>
            {t(`${base}.empty`, {})}
          </Text>
        }
      >
        {projects?.hits.map((project) => (
          <ProjectRow
            key={`${project.source}:${project.id}`}
            project={project}
            gameVersion={gameVersion}
            installed={installedIds.has(`${project.source}:${project.id}`)}
            onInstalled={() =>
              queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.installed() })
            }
          />
        ))}
      </Table>
    </Stack>
  );
}

export default function ServerMinecraftBrowse() {
  const [search, setSearch] = useState('');
  const [query] = useDebouncedValue(search.trim(), 400);

  return (
    <ContentPage search={search} setSearch={setSearch}>
      {(options) => <Browse options={options} query={query} />}
    </ContentPage>
  );
}
