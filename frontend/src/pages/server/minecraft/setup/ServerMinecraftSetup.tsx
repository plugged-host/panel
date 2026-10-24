// plugged.host: the setup a player's new Minecraft server starts with (docs/features/minecraft.md)
import { useDebouncedValue } from '@mantine/hooks';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { Navigate, NavLink } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import completeMinecraftOnboarding from '@/api/server/minecraft/completeMinecraftOnboarding.ts';
import getModpacks from '@/api/server/minecraft/getModpacks.ts';
import getModpackVersions from '@/api/server/minecraft/getModpackVersions.ts';
import searchModpacks from '@/api/server/minecraft/searchModpacks.ts';
import getServerSoftware from '@/api/server/settings/getServerSoftware.ts';
import getSoftwareVersions from '@/api/server/settings/getSoftwareVersions.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Progress from '@/elements/feedback/Progress.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Choice, { ChoiceSection } from '@/elements/input/Choice.tsx';
import Select from '@/elements/input/Select.tsx';
import TagsInput from '@/elements/input/TagsInput.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  minecraftLoaderSchema,
  minecraftOnboardingProgressSchema,
  minecraftOnboardingSchema,
  minecraftProjectSchema,
  minecraftSourceSchema,
} from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { minecraftSourceLabels } from '../content/labels.ts';
import { setupRunning, useMinecraftSetup } from './useMinecraftSetup.ts';

type Onboarding = z.infer<typeof minecraftOnboardingSchema>;
type Loader = z.infer<typeof minecraftLoaderSchema>;
type Project = z.infer<typeof minecraftProjectSchema>;
type Source = z.infer<typeof minecraftSourceSchema>;

const base = 'pages.server.minecraft.setup';
const LATEST = 'latest';
const MAX_NAMES = 20;
const NAME = /^[A-Za-z0-9_]{1,16}$/;

function invalidNames(names: string[]) {
  return names.filter((name) => !NAME.test(name.trim()));
}

function ModpackPicker({
  project,
  setProject,
  versionId,
  setVersionId,
  onLoaders,
}: {
  project: Project | null;
  setProject: (project: Project | null) => void;
  versionId: string | null;
  setVersionId: (version: string | null) => void;
  onLoaders: (loaders: string[]) => void;
}) {
  const { t, language } = useTranslations();
  const server = useServerStore((state) => state.server);

  const [search, setSearch] = useState('');
  const [query] = useDebouncedValue(search.trim(), 400);
  const [source, setSource] = useState<Source>('modrinth');

  const modpacks = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.modpacks(),
    queryFn: () => getModpacks(server.uuid),
  });
  const sources = modpacks.data?.sources ?? ['modrinth'];

  const params = { source, query, page: 1 };
  const results = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.modpackSearch(params),
    queryFn: () => searchModpacks(server.uuid, params),
    enabled: !!modpacks.data?.loaders.length,
    placeholderData: (previous) => previous,
  });

  const versions = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.modpackVersions(project?.source ?? source, project?.id ?? ''),
    queryFn: () => getModpackVersions(server.uuid, project!.source, project!.id),
    enabled: !!project,
  });
  const list = versions.data ?? [];
  const selected = list.find((version) => version.id === versionId) ?? null;

  useEffect(() => {
    if (project && list.length > 0 && !selected) {
      setVersionId(list[0].id);
    }
  }, [project, list, selected]);

  useEffect(() => {
    onLoaders(selected?.loaders ?? []);
  }, [selected]);

  return (
    <Stack gap='sm'>
      <Group gap='sm' align='flex-end'>
        <TextInput
          label={t(`${base}.modpack.search`, {})}
          placeholder={t(`${base}.modpack.searchPlaceholder`, {})}
          value={search}
          onChange={(e) => setSearch(e.currentTarget.value)}
          className='max-w-md flex-1'
        />
        {sources.length > 1 && (
          <Select
            label={t('pages.server.minecraft.content.browse.filter.source', {})}
            data={sources.map((value) => ({ value, label: minecraftSourceLabels[value] }))}
            value={source}
            onChange={(value) => value && setSource(value as Source)}
          />
        )}
      </Group>

      {results.isLoading ? (
        <Spinner.Centered />
      ) : results.error ? (
        <Text size='sm' c='red'>
          {httpErrorToHuman(results.error)}
        </Text>
      ) : (results.data?.hits ?? []).length === 0 ? (
        <Text size='sm' c='dimmed'>
          {t('pages.server.minecraft.content.browse.empty', {})}
        </Text>
      ) : (
        <div className='theme-choice-grid'>
          {results.data?.hits.map((hit) => (
            <Choice
              key={`${hit.source}:${hit.id}`}
              selected={project?.source === hit.source && project.id === hit.id}
              disabled={!hit.installable}
              onSelect={() => {
                setProject(hit);
                setVersionId(null);
              }}
            >
              <span className='theme-choice-title'>
                {hit.icon && <img src={hit.icon} alt='' loading='lazy' className='w-5 h-5 rounded-md shrink-0' />}
                {hit.title}
              </span>
              <span className='theme-choice-text'>
                {t(`${base}.modpack.by`, {
                  author: hit.author,
                  downloads: new Intl.NumberFormat(language, { notation: 'compact' }).format(hit.downloads),
                })}
              </span>
            </Choice>
          ))}
        </div>
      )}

      {project &&
        (versions.isLoading ? (
          <Spinner.Centered />
        ) : list.length === 0 ? (
          <Text size='sm' c='dimmed'>
            {t('pages.server.minecraft.modpacks.modal.noVersions', {})}
          </Text>
        ) : (
          <Select
            label={t(`${base}.modpack.version`, { name: project.title })}
            description={
              selected?.gameVersions.length
                ? t('pages.server.minecraft.modpacks.modal.versionFor', {
                    minecraft: selected.gameVersions.join(', '),
                    loaders: selected.loaders.join(', '),
                  })
                : undefined
            }
            data={list.map((version) => ({
              value: version.id,
              label: version.channel === 'release' ? version.number : `${version.number} (${version.channel})`,
            }))}
            value={selected?.id ?? null}
            onChange={setVersionId}
            allowDeselect={false}
            searchable
            className='max-w-md'
          />
        ))}
    </Stack>
  );
}

function SetupForm({ onboarding }: { onboarding: Onboarding }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);

  const canSoftware = useServerCan('settings.software');
  const canModpack = useServerCan(['settings.software', 'minecraft.content'], false);
  const canBedrock = useServerCan('minecraft.bedrock');

  const software = useQuery({
    queryKey: queryKeys.server(server.uuid).settings.software(),
    queryFn: () => getServerSoftware(server.uuid),
    enabled: canSoftware,
  });
  const modpacks = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.modpacks(),
    queryFn: () => getModpacks(server.uuid),
    enabled: canModpack,
  });
  const choosesSoftware = !!software.data && software.data.variants.length >= 2;
  const choosesVersion = !!software.data && software.data.version !== null;
  const offersModpacks = canModpack && !!modpacks.data?.loaders.length;

  const [mode, setMode] = useState<'software' | 'modpack'>('software');
  const [variant, setVariant] = useState<string | null>(null);
  const [version, setVersion] = useState<string | null>(null);
  const [project, setProject] = useState<Project | null>(null);
  const [modpackVersion, setModpackVersion] = useState<string | null>(null);
  const [modpackLoaders, setModpackLoaders] = useState<string[]>([]);
  const [bedrock, setBedrock] = useState(false);
  const [onlineMode, setOnlineMode] = useState(true);
  const [privateServer, setPrivateServer] = useState(false);
  const [players, setPlayers] = useState<string[]>([]);
  const [operators, setOperators] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!software.data) return;
    setVariant(software.data.current);
    setVersion(software.data.version);
  }, [software.data]);

  const versionVariant = choosesSoftware ? variant : null;
  const versions = useQuery({
    queryKey: queryKeys.server(server.uuid).settings.softwareVersions(versionVariant),
    queryFn: () => getSoftwareVersions(server.uuid, versionVariant),
    enabled: choosesVersion && mode === 'software',
    retry: false,
  });
  const versionData = [
    { value: LATEST, label: t('pages.server.settings.software.versionLatest', {}) },
    ...(versions.data ?? []).map((value) => ({ value, label: value })),
  ];
  if (software.data?.version && !versionData.some((choice) => choice.value === software.data?.version)) {
    versionData.push({ value: software.data.version, label: software.data.version });
  }

  const bedrockPossible =
    canBedrock &&
    (mode === 'modpack'
      ? modpackLoaders.some((loader) => onboarding.bedrockLoaders.includes(loader as Loader))
      : choosesSoftware
        ? onboarding.bedrockVariants.includes(variant ?? '')
        : onboarding.bedrockNow);

  const badNames = [...invalidNames(players), ...invalidNames(operators)];
  const ready =
    (mode === 'software' || (!!project && !!modpackVersion)) &&
    badNames.length === 0 &&
    players.length <= MAX_NAMES &&
    operators.length <= MAX_NAMES;

  const submit = () => {
    if (!ready) return;
    setLoading(true);

    const software_ = software.data;
    completeMinecraftOnboarding(server.uuid, {
      variant: mode === 'software' && choosesSoftware && variant && variant !== software_?.current ? variant : null,
      version: mode === 'software' && choosesVersion && version && version !== software_?.version ? version : null,
      modpack:
        mode === 'modpack' && project && modpackVersion
          ? { source: project.source, project: project.id, version: modpackVersion }
          : null,
      bedrock: bedrock && bedrockPossible,
      onlineMode,
      private: privateServer,
      players: privateServer ? players : [],
      operators,
    })
      .then(() => {
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.onboarding() });
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
        setLoading(false);
      });
  };

  if ((canSoftware && software.isLoading) || (canModpack && modpacks.isLoading)) {
    return <Spinner.Centered />;
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <Stack gap='xl'>
        <Text c='dimmed' size='sm'>
          {t(server.status === 'installing' ? `${base}.introInstalling` : `${base}.intro`, {})}
        </Text>

        {offersModpacks && (
          <ChoiceSection title={t(`${base}.what.title`, {})}>
            <div className='theme-choice-grid'>
              <Choice selected={mode === 'software'} onSelect={() => setMode('software')}>
                <span className='theme-choice-title'>{t(`${base}.what.software`, {})}</span>
                <span className='theme-choice-text'>{t(`${base}.what.softwareText`, {})}</span>
              </Choice>
              <Choice selected={mode === 'modpack'} onSelect={() => setMode('modpack')}>
                <span className='theme-choice-title'>{t(`${base}.what.modpack`, {})}</span>
                <span className='theme-choice-text'>{t(`${base}.what.modpackText`, {})}</span>
              </Choice>
            </div>
          </ChoiceSection>
        )}

        {mode === 'software' && software.data && (choosesSoftware || choosesVersion) && (
          <ChoiceSection title={t(`${base}.software.title`, {})}>
            {choosesSoftware && (
              <div className='theme-choice-grid'>
                {software.data.variants.map((choice) => (
                  <Choice
                    key={choice.value}
                    selected={choice.value === variant}
                    onSelect={() => setVariant(choice.value)}
                  >
                    <span className='theme-choice-title'>{choice.label}</span>
                  </Choice>
                ))}
              </div>
            )}
            {choosesVersion && (
              <Select
                label={t('pages.server.settings.software.versionLabel', {})}
                description={
                  versions.isError
                    ? t('pages.server.settings.software.versionsFailed', {})
                    : t(`${base}.software.version`, {})
                }
                data={versionData}
                value={version}
                onChange={setVersion}
                allowDeselect={false}
                searchable
                className='max-w-md'
              />
            )}
          </ChoiceSection>
        )}

        {mode === 'modpack' && (
          <ChoiceSection title={t(`${base}.modpack.title`, {})}>
            <ModpackPicker
              project={project}
              setProject={setProject}
              versionId={modpackVersion}
              setVersionId={setModpackVersion}
              onLoaders={setModpackLoaders}
            />
          </ChoiceSection>
        )}

        {bedrockPossible && (
          <ChoiceSection title={t(`${base}.bedrock.title`, {})}>
            <div className='theme-choice-grid'>
              <Choice selected={!bedrock} onSelect={() => setBedrock(false)}>
                <span className='theme-choice-title'>{t(`${base}.bedrock.java`, {})}</span>
                <span className='theme-choice-text'>{t(`${base}.bedrock.javaText`, {})}</span>
              </Choice>
              <Choice selected={bedrock} onSelect={() => setBedrock(true)}>
                <span className='theme-choice-title'>{t(`${base}.bedrock.both`, {})}</span>
                <span className='theme-choice-text'>{t(`${base}.bedrock.bothText`, {})}</span>
              </Choice>
            </div>
          </ChoiceSection>
        )}

        <ChoiceSection title={t(`${base}.accounts.title`, {})}>
          <div className='theme-choice-grid'>
            <Choice selected={onlineMode} onSelect={() => setOnlineMode(true)}>
              <span className='theme-choice-title'>{t(`${base}.accounts.premium`, {})}</span>
              <span className='theme-choice-text'>{t(`${base}.accounts.premiumText`, {})}</span>
            </Choice>
            <Choice selected={!onlineMode} onSelect={() => setOnlineMode(false)}>
              <span className='theme-choice-title'>{t(`${base}.accounts.cracked`, {})}</span>
              <span className='theme-choice-text'>{t(`${base}.accounts.crackedText`, {})}</span>
            </Choice>
          </div>
        </ChoiceSection>

        <ChoiceSection title={t(`${base}.access.title`, {})}>
          <div className='theme-choice-grid'>
            <Choice selected={!privateServer} onSelect={() => setPrivateServer(false)}>
              <span className='theme-choice-title'>{t(`${base}.access.public`, {})}</span>
              <span className='theme-choice-text'>{t(`${base}.access.publicText`, {})}</span>
            </Choice>
            <Choice selected={privateServer} onSelect={() => setPrivateServer(true)}>
              <span className='theme-choice-title'>{t(`${base}.access.private`, {})}</span>
              <span className='theme-choice-text'>{t(`${base}.access.privateText`, {})}</span>
            </Choice>
          </div>
        </ChoiceSection>

        <ChoiceSection title={t(`${base}.players.title`, {})}>
          {privateServer && (
            <TagsInput
              label={t(`${base}.players.whitelist`, {})}
              description={t(`${base}.players.whitelistDescription`, { max: MAX_NAMES })}
              placeholder={t(`${base}.players.placeholder`, {})}
              value={players}
              onChange={setPlayers}
              invalidTags={invalidNames(players)}
              allowReordering={false}
            />
          )}
          <TagsInput
            label={t(`${base}.players.operators`, {})}
            description={t(`${base}.players.operatorsDescription`, { max: MAX_NAMES })}
            placeholder={t(`${base}.players.placeholder`, {})}
            value={operators}
            onChange={setOperators}
            invalidTags={invalidNames(operators)}
            allowReordering={false}
          />
          {badNames.length > 0 && (
            <Text size='sm' c='red'>
              {t(`${base}.players.invalid`, { names: badNames.join(', ') })}
            </Text>
          )}
        </ChoiceSection>

        <Group>
          <Button type='submit' disabled={!ready} loading={loading}>
            {t(`${base}.submit`, {})}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

function SetupProgress({ progress }: { progress: z.infer<typeof minecraftOnboardingProgressSchema> }) {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const running = setupRunning(progress);

  return (
    <Stack>
      {progress.step === 'failed' ? (
        <Text c='red'>{t(`${base}.progress.failed`, { error: progress.error ?? '' })}</Text>
      ) : (
        <Text>{t(`${base}.progress.${progress.step}`, {})}</Text>
      )}
      {running && <Progress value={NaN} indeterminate />}

      {progress.step === 'done' && progress.error && (
        <Text size='sm'>{t(`${base}.progress.problems`, { problems: progress.error })}</Text>
      )}
      {progress.skipped.length > 0 && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.progress.skipped`, { names: progress.skipped.join(', ') })}
        </Text>
      )}

      {!running && (
        <Group>
          <NavLink to={`/server/${server.uuidShort}`}>
            <Button>{t(`${base}.progress.console`, {})}</Button>
          </NavLink>
        </Group>
      )}
    </Stack>
  );
}

export default function ServerMinecraftSetup() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const setup = useMinecraftSetup();

  let content;
  if (setup.isLoading) {
    content = <Spinner.Centered />;
  } else if (setup.data?.pending) {
    content = <SetupForm onboarding={setup.data} />;
  } else if (setup.data?.progress) {
    content = <SetupProgress progress={setup.data.progress} />;
  } else {
    return <Navigate to={`/server/${server.uuidShort}`} replace />;
  }

  return <ServerContentContainer title={t(`${base}.title`, {})}>{content}</ServerContentContainer>;
}
