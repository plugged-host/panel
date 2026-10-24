// plugged.host: finding and installing a modpack, which brings its own loader and version (docs/features/minecraft.md)
import { faArrowUpRightFromSquare } from '@fortawesome/free-solid-svg-icons';
import { useDebouncedValue } from '@mantine/hooks';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { z } from 'zod';
import getModpacks from '@/api/server/minecraft/getModpacks.ts';
import searchModpacks, { ModpackSearchParams } from '@/api/server/minecraft/searchModpacks.ts';
import Button from '@/elements/buttons/Button.tsx';
import { ServerCan } from '@/elements/Can.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Progress from '@/elements/feedback/Progress.tsx';
import Select from '@/elements/input/Select.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  minecraftModpackInstallSchema,
  minecraftProjectSchema,
  minecraftSourceSchema,
} from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ContentPage from './ContentPage.tsx';
import { minecraftLoaderLabels, minecraftSourceLabels } from './labels.ts';
import ModpackModal from './ModpackModal.tsx';

type Source = z.infer<typeof minecraftSourceSchema>;
type Install = z.infer<typeof minecraftModpackInstallSchema>;

const base = 'pages.server.minecraft.modpacks';

function runningStep(step: Install['step'] | undefined) {
  return !!step && step !== 'done' && step !== 'failed';
}

function installing(install: Install | null | undefined) {
  return runningStep(install?.step);
}

function InstallState({ install }: { install: Install }) {
  const { t } = useTranslations();

  if (install.step === 'failed') {
    return (
      <Text size='sm' c='red'>
        {t(`${base}.state.failed`, { name: install.name, error: install.error ?? '' })}
      </Text>
    );
  }

  if (install.step === 'done') {
    return (
      <Stack gap={4}>
        <Text size='sm'>{t(`${base}.state.done`, { name: install.name })}</Text>
        {install.skipped.length > 0 && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.state.skipped`, { files: install.skipped.join(', ') })}
          </Text>
        )}
        {install.overridesSkipped && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.state.overridesSkipped`, {})}
          </Text>
        )}
      </Stack>
    );
  }

  const software =
    install.loader && install.minecraft
      ? t(`${base}.state.software`, { loader: minecraftLoaderLabels[install.loader], version: install.minecraft })
      : '';

  return (
    <Stack gap={4}>
      <Text size='sm'>
        {t(`${base}.state.installing`, { name: install.name })} {t(`${base}.step.${install.step}`, {})}{' '}
        {install.step === 'software' ? software : ''}
      </Text>
      <Progress
        value={install.step === 'files' && install.total > 0 ? (install.files / install.total) * 100 : NaN}
        indeterminate={install.step !== 'files' || install.total === 0}
      />
    </Stack>
  );
}

function ModpackRow({
  project,
  disabled,
  onStarted,
}: {
  project: z.infer<typeof minecraftProjectSchema>;
  disabled: boolean;
  onStarted: () => void;
}) {
  const { t, language } = useTranslations();
  const [open, setOpen] = useState(false);
  const canInstall = useServerCan(['minecraft.content', 'settings.software'], false);

  return (
    <>
      {canInstall && project.installable && (
        <ModpackModal project={project} opened={open} onClose={() => setOpen(false)} onStarted={onStarted} />
      )}

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faArrowUpRightFromSquare,
            label: t('pages.server.minecraft.content.button.open', { source: minecraftSourceLabels[project.source] }),
            onClick: () => window.open(project.url, '_blank', 'noopener,noreferrer'),
            color: 'gray',
          },
        ]}
      >
        {({ items, openMenu }) => (
          <TableRow
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(e.clientX, e.clientY);
            }}
          >
            <TableData>
              <Group gap='sm' wrap='nowrap'>
                {project.icon && (
                  <img src={project.icon} alt='' loading='lazy' className='w-10 h-10 rounded-md shrink-0' />
                )}
                <Stack gap={0}>
                  <Text size='sm' fw={500}>
                    {project.title}
                  </Text>
                  <Text size='xs' c='dimmed' lineClamp={2}>
                    {project.description}
                  </Text>
                </Stack>
              </Group>
            </TableData>

            <TableData>{project.author}</TableData>

            <TableData>{new Intl.NumberFormat(language, { notation: 'compact' }).format(project.downloads)}</TableData>

            <TableData>
              {canInstall && (
                <Button
                  color='gray'
                  size='xs'
                  disabled={disabled || !project.installable}
                  onClick={() => setOpen(true)}
                >
                  {t(`${base}.button.install`, {})}
                </Button>
              )}
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}

function Modpacks({ query }: { query: string }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);

  const modpacks = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.modpacks(),
    queryFn: () => getModpacks(server.uuid),
    // an install runs after its request: poll it until it ends
    refetchInterval: (current) => (installing(current.state.data?.install) ? 3000 : false),
  });
  const install = modpacks.data?.install ?? null;

  const step = install?.step;
  const previousStep = useRef(step);
  useEffect(() => {
    if (install && runningStep(previousStep.current)) {
      if (step === 'done') {
        addToast(t(`${base}.state.done`, { name: install.name }), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.installed() });
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.content() });
      } else if (step === 'failed') {
        addToast(t(`${base}.state.failed`, { name: install.name, error: install.error ?? '' }), 'error');
      }
    }
    previousStep.current = step;
  }, [step]);

  const sources = modpacks.data?.sources ?? ['modrinth'];
  const [source, setSource] = useState<Source>('modrinth');
  const [page, setPage] = useState(1);
  useEffect(() => setPage(1), [source, query]);

  const params: ModpackSearchParams = { source, query, page };
  const results = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.modpackSearch(params),
    queryFn: () => searchModpacks(server.uuid, params),
    keepPrevious: true,
    enabled: !!modpacks.data?.loaders.length,
  });

  if (modpacks.data && modpacks.data.loaders.length === 0) {
    return (
      <Text size='sm' c='dimmed'>
        {t(`${base}.noLoaders`, {})}
      </Text>
    );
  }

  const busy = !modpacks.data || modpacks.data.running || installing(install);

  return (
    <Stack gap='sm'>
      {install && <InstallState install={install} />}
      {modpacks.data?.running && !installing(install) && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.running`, {})}
        </Text>
      )}
      {modpacks.data && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.loaders`, {
            loaders: modpacks.data.loaders.map((loader) => minecraftLoaderLabels[loader]).join(', '),
          })}
        </Text>
      )}

      {sources.length > 1 && (
        <Group gap='sm' align='flex-end'>
          <Select
            label={t('pages.server.minecraft.content.browse.filter.source', {})}
            data={sources.map((value) => ({ value, label: minecraftSourceLabels[value] }))}
            value={source}
            onChange={(value) => value && setSource(value as Source)}
          />
        </Group>
      )}

      <Table
        columns={[
          t('pages.server.minecraft.content.browse.table.project', {}),
          t('common.table.columns.author', {}),
          t('pages.server.minecraft.content.browse.table.downloads', {}),
          '',
          '',
        ]}
        loading={results.loading}
        pagination={results.data ? { ...results.data, data: results.data.hits } : undefined}
        onPageSelect={setPage}
        empty={
          <Text size='sm' c='dimmed'>
            {t('pages.server.minecraft.content.browse.empty', {})}
          </Text>
        }
      >
        {results.data?.hits.map((project) => (
          <ModpackRow
            key={`${project.source}:${project.id}`}
            project={project}
            disabled={busy}
            onStarted={() => modpacks.refetch()}
          />
        ))}
      </Table>
    </Stack>
  );
}

export default function ServerMinecraftModpacks() {
  const [search, setSearch] = useState('');
  const [query] = useDebouncedValue(search.trim(), 400);

  return (
    <ContentPage search={search} setSearch={setSearch} requireLoader={false}>
      {() => (
        <ServerCan action='minecraft.content'>
          <Modpacks query={query} />
        </ServerCan>
      )}
    </ContentPage>
  );
}
