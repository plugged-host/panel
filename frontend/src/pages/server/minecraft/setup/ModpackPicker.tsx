// plugged.host: the setup's modpack choice: popular packs at once, searched as typed, in the Modpacks
// tab's catalog rows; a click picks a pack with its newest version (docs/features/minecraft.md)
import { faMagnifyingGlass } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useDebouncedValue } from '@mantine/hooks';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { KeyboardEvent, useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getModpackVersions from '@/api/server/minecraft/getModpackVersions.ts';
import searchModpacks, { ModpackSearchParams } from '@/api/server/minecraft/searchModpacks.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import SegmentedControl from '@/elements/layout/SegmentedControl.tsx';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  minecraftProjectSchema,
  minecraftSourceSchema,
  minecraftVersionSchema,
} from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ContentIcon from '../content/ContentIcon.tsx';
import { minecraftSourceLabels, minecraftVersionLabel } from '../content/labels.ts';

type Project = z.infer<typeof minecraftProjectSchema>;
type Version = z.infer<typeof minecraftVersionSchema>;
type Source = z.infer<typeof minecraftSourceSchema>;

const base = 'pages.server.minecraft.setup.modpack';
const browse = 'pages.server.minecraft.content.browse';
const SEARCH_DEBOUNCE_MS = 400;

const searchPlaceholders = {
  modrinth: `${browse}.search`,
  curseforge: `${browse}.searchCurseforge`,
  spigotmc: `${browse}.searchSpigotmc`,
} as const;

function PackRow({
  project,
  selected,
  picking,
  onPick,
}: {
  project: Project;
  selected: boolean;
  picking: boolean;
  onPick?: () => void;
}) {
  const { t, language } = useTranslations();
  const pickable = project.installable && !!onPick && !picking;
  const keyDown = (event: KeyboardEvent) => {
    if (pickable && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      onPick?.();
    }
  };

  return (
    <TableRow
      className='theme-setup-pack'
      data-selected={selected || undefined}
      data-disabled={!project.installable || undefined}
      role={onPick ? 'button' : undefined}
      tabIndex={pickable ? 0 : undefined}
      aria-pressed={onPick ? selected : undefined}
      aria-disabled={onPick && !pickable ? true : undefined}
      onClick={pickable ? onPick : undefined}
      onKeyDown={keyDown}
    >
      <TableData>
        <ContentIcon icon={project.icon} kind='modpack' />
      </TableData>
      <TableData className='theme-catalog-project'>
        <span className='theme-catalog-title'>{project.title}</span>
        {project.description && <span className='theme-catalog-description'>{project.description}</span>}
      </TableData>
      <TableData>{project.downloads.toLocaleString(intlLocale(language))}</TableData>
      <TableData>
        {!project.installable && (
          <Badge color='gray' size='sm'>
            {t(`${browse}.badge.external`, {})}
          </Badge>
        )}
      </TableData>
    </TableRow>
  );
}

/** The picked pack with its version, or the catalog to pick one from. */
export default function ModpackPicker({
  sources,
  project,
  version,
  onPick,
  onClear,
}: {
  sources: Source[];
  project: Project | null;
  version: Version | null;
  onPick: (project: Project, version: Version) => void;
  onClear: () => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);

  const [source, setSource] = useState<Source>(sources[0] ?? 'modrinth');
  const [search, setSearch] = useState('');
  const [query] = useDebouncedValue(search.trim(), SEARCH_DEBOUNCE_MS);
  const [page, setPage] = useState(1);
  const [picking, setPicking] = useState<string | null>(null);

  useEffect(() => setPage(1), [source, query]);

  const params: ModpackSearchParams = { source, query, page };
  const results = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.modpackSearch({ ...params, setup: true }),
    queryFn: () => searchModpacks(server.uuid, params),
    keepPrevious: true,
    enabled: !project,
  });

  const versions = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.modpackVersions(project?.source ?? '', project?.id ?? ''),
    queryFn: () => (project ? getModpackVersions(server.uuid, project.source, project.id) : Promise.resolve([])),
    enabled: !!project,
  });

  // the newest version the server can run, so a pack is one click
  const pick = (chosen: Project) => {
    setPicking(chosen.id);
    queryClient
      .fetchQuery({
        queryKey: queryKeys.server(server.uuid).minecraft.modpackVersions(chosen.source, chosen.id),
        queryFn: () => getModpackVersions(server.uuid, chosen.source, chosen.id),
      })
      .then((list) => {
        if (list.length === 0) {
          addToast(t('pages.server.minecraft.modpacks.modal.noVersions', {}), 'error');
          return;
        }
        onPick(chosen, list[0]);
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setPicking(null));
  };

  if (project) {
    const list = versions.data ?? (version ? [version] : []);

    return (
      <div className='theme-setup-field'>
        <span className='theme-setup-label'>{t(`${base}.label`, {})}</span>
        <Table columns={['', t(`${browse}.table.project`, {}), t(`${browse}.table.downloads`, {}), '']} flush>
          <PackRow project={project} selected picking={false} />
        </Table>
        <div className='theme-setup-pack-options'>
          <Select
            size='xs'
            label={t(`${base}.version`, {})}
            data={list.map((choice) => ({ value: choice.id, label: minecraftVersionLabel(choice) }))}
            value={version?.id ?? null}
            onChange={(id) => {
              const chosen = list.find((choice) => choice.id === id);
              if (chosen) onPick(project, chosen);
            }}
            searchable
          />
          <button type='button' className='theme-link-action' onClick={onClear}>
            {t(`${base}.another`, {})}
          </button>
        </div>
      </div>
    );
  }

  const found = results.data;

  return (
    <div className='theme-setup-field'>
      <span className='theme-setup-label'>{t(`${base}.label`, {})}</span>
      <div className='theme-table-toolbar'>
        <TextInput
          placeholder={t(searchPlaceholders[source], {})}
          aria-label={t(searchPlaceholders[source], {})}
          value={search}
          onChange={(event) => setSearch(event.currentTarget.value)}
          leftSection={<FontAwesomeIcon icon={faMagnifyingGlass} />}
          className='theme-header-search'
        />
        {sources.length > 1 && (
          <SegmentedControl
            aria-label={t(`${browse}.filter.source`, {})}
            data={sources.map((value) => ({ value, label: minecraftSourceLabels[value] }))}
            value={source}
            onChange={(value) => setSource(value as Source)}
          />
        )}
      </div>
      <Table
        columns={['', t(`${browse}.table.project`, {}), t(`${browse}.table.downloads`, {}), '']}
        loading={results.loading}
        pagination={found ? { ...found, data: found.hits } : undefined}
        onPageSelect={setPage}
        empty={
          <EmptyState
            flush
            icon={faMagnifyingGlass}
            title={t(`${browse}.emptyHeading`, {})}
            description={t(`${browse}.empty`, {})}
          />
        }
      >
        {found?.hits.map((hit) => (
          <PackRow
            key={`${hit.source}:${hit.id}`}
            project={hit}
            selected={false}
            picking={picking !== null}
            onPick={() => pick(hit)}
          />
        ))}
      </Table>
    </div>
  );
}
