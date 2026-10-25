// plugged.host: the catalog under the installed list: Modrinth, CurseForge and SpigotMC, plugins, mods
// and modpacks, as the Pelican panel's content table (docs/features/minecraft.md)
import { faFilter, faMagnifyingGlass, faTableColumns, faXmark } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useDebouncedValue } from '@mantine/hooks';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { z } from 'zod';
import getInstalledContent from '@/api/server/minecraft/getInstalledContent.ts';
import searchContent, { ContentSearchParams } from '@/api/server/minecraft/searchContent.ts';
import searchModpacks, { ModpackSearchParams } from '@/api/server/minecraft/searchModpacks.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Table from '@/elements/data-display/Table.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Popover from '@/elements/overlays/Popover.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftKindSchema, minecraftSourceSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { minecraftSourceLabels } from './labels.ts';
import ProjectRow, { ProjectColumns } from './ProjectRow.tsx';
import { ContentOptions } from './useContentOptions.ts';
import { modpackInstalling, useModpacks } from './useModpacks.ts';

type Source = z.infer<typeof minecraftSourceSchema>;
type Kind = z.infer<typeof minecraftKindSchema>;

// a filter left empty (its indicator removed) falls back to the first choice, or any version
interface Filters {
  source: Source | null;
  kind: Kind | null;
  version: string | null;
}

const base = 'pages.server.minecraft.content.browse';
const SEARCH_DEBOUNCE_MS = 600;

const searchPlaceholders = {
  modrinth: `${base}.search`,
  curseforge: `${base}.searchCurseforge`,
  spigotmc: `${base}.searchSpigotmc`,
} as const;

export default function BrowseTable({ options }: { options: ContentOptions }) {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);

  const modpacks = useModpacks(options.modpacks);

  // modpacks bring a loader and a version of their own, so they have their own search
  const kinds: Kind[] = [
    ...options.kinds.filter((kind) => kind !== 'modpack'),
    ...(options.modpacks ? (['modpack'] as const) : []),
  ];
  const versions = useMemo(() => {
    const list = [...options.gameVersions];
    if (options.gameVersion && !list.includes(options.gameVersion)) {
      list.unshift(options.gameVersion);
    }
    return list;
  }, [options]);

  const defaults: Filters = { source: 'modrinth', kind: kinds[0] ?? null, version: options.gameVersion };
  const [applied, setApplied] = useState<Filters>(defaults);
  const [draft, setDraft] = useState<Filters>(defaults);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [columns, setColumns] = useState<ProjectColumns>({ author: true, categories: true });
  const [search, setSearch] = useState('');
  const [query] = useDebouncedValue(search.trim(), SEARCH_DEBOUNCE_MS);
  const [page, setPage] = useState(1);

  const kind = applied.kind ?? kinds[0] ?? null;
  const sources: Source[] = kind === 'modpack' ? (modpacks.data?.sources ?? ['modrinth']) : options.sources;
  const source = applied.source && sources.includes(applied.source) ? applied.source : (sources[0] ?? 'modrinth');

  useEffect(() => setPage(1), [applied, query]);

  const contentParams: ContentSearchParams = {
    source,
    kind,
    query,
    // '' is any version
    gameVersion: applied.version ?? '',
    page,
  };
  const modpackParams: ModpackSearchParams = { source, query, gameVersion: applied.version, page };

  const results = useResource({
    queryKey:
      kind === 'modpack'
        ? queryKeys.server(server.uuid).minecraft.modpackSearch(modpackParams)
        : queryKeys.server(server.uuid).minecraft.search(contentParams),
    queryFn: () =>
      kind === 'modpack'
        ? searchModpacks(server.uuid, modpackParams)
        : searchContent(server.uuid, contentParams).then((found) => found.projects),
    keepPrevious: true,
    enabled: kind !== null,
  });

  const installed = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.installed(),
    queryFn: () => getInstalledContent(server.uuid),
    silent: true,
    enabled: kind !== 'modpack',
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

  const modpackBlocked = !modpacks.data
    ? 'busy'
    : modpackInstalling(modpacks.data.install)
      ? 'busy'
      : modpacks.data.running
        ? 'running'
        : null;

  const indicators = [
    sources.length > 1 && applied.source
      ? {
          key: 'source',
          label: `${t(`${base}.filter.source`, {})}: ${minecraftSourceLabels[source]}`,
          clear: () => setApplied({ ...applied, source: null }),
        }
      : null,
    kinds.length > 1 && applied.kind
      ? {
          key: 'kind',
          label: `${t(`${base}.filter.kind`, {})}: ${t(`${base}.kind.${applied.kind}`, {})}`,
          clear: () => setApplied({ ...applied, kind: null }),
        }
      : null,
    applied.version
      ? {
          key: 'version',
          label: `${t(`${base}.filter.version`, {})}: ${applied.version}`,
          clear: () => setApplied({ ...applied, version: null }),
        }
      : null,
  ].filter((indicator) => indicator !== null);

  const projects = results.data;

  return (
    <section>
      <div className='theme-table-toolbar'>
        <TextInput
          placeholder={t(searchPlaceholders[source], {})}
          value={search}
          onChange={(e) => setSearch(e.currentTarget.value)}
          leftSection={<FontAwesomeIcon icon={faMagnifyingGlass} />}
          className='theme-header-search'
        />

        <Popover opened={filtersOpen} onChange={setFiltersOpen} position='bottom-end' width={320}>
          <Popover.Target>
            <Tooltip label={t(`${base}.filters.trigger`, {})} disabled={filtersOpen}>
              <ActionIcon
                size='input-sm'
                variant='subtle'
                color='blue'
                aria-label={t(`${base}.filters.trigger`, {})}
                onClick={() => {
                  // Filament's filters are deferred: the form starts from what is applied
                  if (!filtersOpen) setDraft(applied);
                  setFiltersOpen(!filtersOpen);
                }}
              >
                <FontAwesomeIcon icon={faFilter} />
              </ActionIcon>
            </Tooltip>
          </Popover.Target>
          <Popover.Dropdown>
            <div className='theme-catalog-filters'>
              <div className='theme-catalog-filters-head'>
                <h4 className='theme-eco-heading'>{t(`${base}.filters.heading`, {})}</h4>
                <button
                  type='button'
                  className='theme-link-action'
                  data-danger
                  onClick={() => {
                    setApplied(defaults);
                    setDraft(defaults);
                    setFiltersOpen(false);
                  }}
                >
                  {t(`${base}.filters.reset`, {})}
                </button>
              </div>

              {sources.length > 1 && (
                <Select
                  label={t(`${base}.filter.source`, {})}
                  data={sources.map((value) => ({ value, label: minecraftSourceLabels[value] }))}
                  value={draft.source && sources.includes(draft.source) ? draft.source : sources[0]}
                  onChange={(value) => setDraft({ ...draft, source: value as Source })}
                  comboboxProps={{ withinPortal: false }}
                />
              )}
              {kinds.length > 1 && (
                <Select
                  label={t(`${base}.filter.kind`, {})}
                  data={kinds.map((value) => ({ value, label: t(`${base}.kind.${value}`, {}) }))}
                  value={draft.kind ?? kinds[0]}
                  onChange={(value) => setDraft({ ...draft, kind: value as Kind })}
                  comboboxProps={{ withinPortal: false }}
                />
              )}
              {versions.length > 0 && (
                <Select
                  label={t(`${base}.filter.version`, {})}
                  data={versions}
                  value={draft.version}
                  onChange={(value) => setDraft({ ...draft, version: value })}
                  searchable
                  clearable
                  allowDeselect
                  comboboxProps={{ withinPortal: false }}
                />
              )}

              <Button
                fullWidth
                onClick={() => {
                  setApplied(draft);
                  setFiltersOpen(false);
                }}
              >
                {t(`${base}.filters.apply`, {})}
              </Button>
            </div>
          </Popover.Dropdown>
        </Popover>

        <Popover position='bottom-end' width={220}>
          <Popover.Target>
            <Tooltip label={t(`${base}.columns.trigger`, {})}>
              <ActionIcon size='input-sm' variant='subtle' color='gray' aria-label={t(`${base}.columns.trigger`, {})}>
                <FontAwesomeIcon icon={faTableColumns} />
              </ActionIcon>
            </Tooltip>
          </Popover.Target>
          <Popover.Dropdown>
            <div className='theme-catalog-filters'>
              <h4 className='theme-eco-heading'>{t(`${base}.columns.heading`, {})}</h4>
              <Checkbox
                label={t('common.table.columns.author', {})}
                checked={columns.author}
                onChange={(e) => setColumns({ ...columns, author: e.currentTarget.checked })}
              />
              <Checkbox
                label={t(`${base}.table.categories`, {})}
                checked={columns.categories}
                onChange={(e) => setColumns({ ...columns, categories: e.currentTarget.checked })}
              />
            </div>
          </Popover.Dropdown>
        </Popover>
      </div>

      {indicators.length > 0 && (
        <div className='theme-catalog-indicators'>
          <span>{t(`${base}.filters.indicator`, {})}</span>
          {indicators.map((indicator) => (
            <Badge
              key={indicator.key}
              color='gray'
              size='md'
              rightSection={
                <button
                  type='button'
                  className='theme-catalog-indicator-remove'
                  aria-label={t(`${base}.filters.remove`, {})}
                  onClick={indicator.clear}
                >
                  <FontAwesomeIcon icon={faXmark} />
                </button>
              }
            >
              {indicator.label}
            </Badge>
          ))}
        </div>
      )}

      <Table
        columns={[
          '',
          t(`${base}.table.project`, {}),
          ...(columns.author ? [t('common.table.columns.author', {})] : []),
          t(`${base}.table.downloads`, {}),
          ...(columns.categories ? [t(`${base}.table.categories`, {})] : []),
          '',
          '',
        ]}
        loading={results.loading}
        pagination={projects ? { ...projects, data: projects.hits } : undefined}
        onPageSelect={setPage}
        empty={
          <EmptyState
            flush
            icon={faMagnifyingGlass}
            title={t(`${base}.emptyHeading`, {})}
            description={t(`${base}.empty`, {})}
          />
        }
      >
        {projects?.hits.map((project) => (
          <ProjectRow
            key={`${project.source}:${project.id}`}
            project={project}
            columns={columns}
            gameVersion={applied.version ?? options.gameVersion}
            installed={installedIds.has(`${project.source}:${project.id}`)}
            modpackBlocked={modpackBlocked}
            onInstalled={() =>
              queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.installed() })
            }
            onModpackStarted={() => modpacks.refetch()}
          />
        ))}
      </Table>
    </section>
  );
}
