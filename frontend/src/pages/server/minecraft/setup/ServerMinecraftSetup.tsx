// plugged.host: the setup a player's new Minecraft server starts with, the Pelican panel's setup
// wizard (ServerOnboardingPage, docs/features/minecraft.md)
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ReactNode, useEffect, useState } from 'react';
import { Navigate, useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import completeMinecraftOnboarding from '@/api/server/minecraft/completeMinecraftOnboarding.ts';
import getModpackVersions from '@/api/server/minecraft/getModpackVersions.ts';
import searchModpacks from '@/api/server/minecraft/searchModpacks.ts';
import getServerSoftware from '@/api/server/settings/getServerSoftware.ts';
import getSoftwareVersions from '@/api/server/settings/getSoftwareVersions.ts';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Choice from '@/elements/input/Choice.tsx';
import Select from '@/elements/input/Select.tsx';
import TagsInput from '@/elements/input/TagsInput.tsx';
import Text from '@/elements/typography/Text.tsx';
import Wizard, { WizardStep } from '@/elements/Wizard.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  minecraftLoaderSchema,
  minecraftOnboardingSchema,
  minecraftProjectSchema,
} from '@/lib/schemas/server/minecraft.ts';
import { useSearchableResource } from '@/plugins/resource/useSearchableResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useModpacks } from '../content/useModpacks.ts';
import { useMinecraftSetup } from './useMinecraftSetup.ts';

type Onboarding = z.infer<typeof minecraftOnboardingSchema>;
type Loader = z.infer<typeof minecraftLoaderSchema>;
type Project = z.infer<typeof minecraftProjectSchema>;

const base = 'pages.server.minecraft.setup';
const LATEST = 'latest';
const MAX_NAMES = 20;
const NAME = /^[A-Za-z0-9_]{1,16}$/;
// Filament's helper text sits under its field
const HELPER_BELOW: ('label' | 'input' | 'description' | 'error')[] = ['label', 'input', 'description', 'error'];

// the Pelican panel's catalog descriptions, by the software's family
const SOFTWARE_DESCRIPTIONS = {
  plugins: `${base}.software.descriptions.plugins`,
  fabric: `${base}.software.descriptions.fabric`,
  neoforge: `${base}.software.descriptions.neoforge`,
  forge: `${base}.software.descriptions.forge`,
} as const;
const PLUGIN_SOFTWARE = ['paper', 'purpur', 'folia', 'spigot', 'bukkit', 'pufferfish', 'leaf', 'vanilla'];

function softwareDescription(value: string): keyof typeof SOFTWARE_DESCRIPTIONS | null {
  const name = value.trim().toLowerCase();
  if (PLUGIN_SOFTWARE.includes(name)) return 'plugins';
  if (name === 'fabric' || name === 'neoforge' || name === 'forge') return name;
  return null;
}

function invalidNames(names: string[]) {
  return names.filter((name) => !NAME.test(name.trim()));
}

/** One pick-one card, the Pelican panel's CardSelect option (label over description). */
function SetupCard({
  selected,
  label,
  description,
  onSelect,
}: {
  selected: boolean;
  label: string;
  description?: string;
  onSelect: () => void;
}) {
  return (
    <Choice radio selected={selected} onSelect={onSelect}>
      <span className='theme-choice-title'>{label}</span>
      {description && <span className='theme-choice-text'>{description}</span>}
    </Choice>
  );
}

function SetupCards({ label, children }: { label?: string; children: ReactNode }) {
  return (
    <div className='theme-setup-field'>
      {label && <span className='theme-setup-label'>{label}</span>}
      <div className='theme-setup-cards' role='group' aria-label={label}>
        {children}
      </div>
    </div>
  );
}

function SetupNote({ children }: { children: ReactNode }) {
  return (
    <Text size='sm' c='dimmed'>
      {children}
    </Text>
  );
}

/** A modpack from the first source, searched as it is typed, for the chosen loader. */
function ModpackSelect({
  loader,
  project,
  onChange,
}: {
  loader: Loader | null;
  project: Project | null;
  onChange: (project: Project | null) => void;
}) {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const modpacks = useModpacks();
  const source = modpacks.data?.sources[0] ?? 'modrinth';

  const packs = useSearchableResource<Project>({
    queryKey: queryKeys.server(server.uuid).minecraft.modpackSearch({ source, loader, setup: true }),
    fetcher: (query) =>
      searchModpacks(server.uuid, { source, query, loader, page: 1 }).then((found) => ({
        ...found,
        data: found.hits,
      })),
    debounceMs: 400,
  });

  const data = packs.items.map((pack) => ({ value: pack.id, label: pack.title }));
  if (project && !packs.items.some((pack) => pack.id === project.id)) {
    data.unshift({ value: project.id, label: project.title });
  }

  return (
    <Select
      label={t(`${base}.modpack.label`, {})}
      placeholder={t(`${base}.modpack.none`, {})}
      description={t(`${base}.modpack.hint`, {})}
      inputWrapperOrder={HELPER_BELOW}
      nothingFoundMessage={packs.search ? t(`${base}.modpack.nothing`, {}) : t(`${base}.modpack.search`, {})}
      data={data}
      value={project?.id ?? null}
      onChange={(value) => onChange(packs.items.find((pack) => pack.id === value) ?? (value ? project : null))}
      filter={({ options }) => options}
      searchable
      clearable
      allowDeselect
      searchValue={packs.search}
      onSearchChange={packs.setSearch}
      loading={packs.loading}
    />
  );
}

function ModpackVersionSelect({
  project,
  value,
  onChange,
}: {
  project: Project;
  value: string | null;
  onChange: (version: string | null) => void;
}) {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const versions = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.modpackVersions(project.source, project.id),
    queryFn: () => getModpackVersions(server.uuid, project.source, project.id),
  });
  const list = versions.data ?? [];

  // the newest build is chosen for the player, so a pack is one click
  useEffect(() => {
    if (list.length > 0 && !list.some((version) => version.id === value)) {
      onChange(list[0].id);
    }
  }, [list, value]);

  if (versions.isLoading) return <Spinner.Centered />;
  if (list.length === 0) {
    return <SetupNote>{t('pages.server.minecraft.modpacks.modal.noVersions', {})}</SetupNote>;
  }

  return (
    <Select
      label={t(`${base}.modpack.version`, {})}
      description={t(`${base}.modpack.versionHint`, {})}
      inputWrapperOrder={HELPER_BELOW}
      data={list.map((version) => ({
        value: version.id,
        label: `${version.name || version.number}${version.gameVersions[0] ? ` — ${version.gameVersions[0]}` : ''}`,
      }))}
      value={value}
      onChange={onChange}
    />
  );
}

function SetupForm({ onboarding }: { onboarding: Onboarding }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const powerState = useServerStore((state) => state.state);

  const canSoftware = useServerCan('settings.software');
  const canModpack = useServerCan(['settings.software', 'minecraft.content'], false);
  const canBedrock = useServerCan('minecraft.bedrock');

  const software = useQuery({
    queryKey: queryKeys.server(server.uuid).settings.software(),
    queryFn: () => getServerSoftware(server.uuid),
    enabled: canSoftware,
  });
  const modpacks = useModpacks(canModpack);
  const choosesSoftware = !!software.data && software.data.variants.length >= 2;
  const choosesVersion = !!software.data && software.data.version !== null;

  const [active, setActive] = useState(0);
  const [variant, setVariant] = useState<string | null>(null);
  const [version, setVersion] = useState<string | null>(null);
  const [project, setProject] = useState<Project | null>(null);
  const [modpackVersion, setModpackVersion] = useState<string | null>(null);
  const [bedrock, setBedrock] = useState(false);
  const [onlineMode, setOnlineMode] = useState(true);
  // private first, as the Pelican panel: a public server is found by scanners within hours
  const [privateServer, setPrivateServer] = useState(true);
  const [players, setPlayers] = useState<string[]>([]);
  const [operators, setOperators] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!software.data) return;
    setVariant(software.data.current ?? software.data.variants[0]?.value ?? null);
    setVersion(software.data.version);
  }, [software.data]);

  const versionVariant = choosesSoftware ? variant : null;
  const versions = useQuery({
    queryKey: queryKeys.server(server.uuid).settings.softwareVersions(versionVariant),
    queryFn: () => getSoftwareVersions(server.uuid, versionVariant),
    enabled: choosesVersion,
    retry: false,
  });
  const versionData = [
    { value: LATEST, label: t(`${base}.software.latest`, {}) },
    ...(versions.data ?? []).filter((value) => value !== LATEST).map((value) => ({ value, label: value })),
  ];
  if (version && !versionData.some((choice) => choice.value === version)) {
    versionData.push({ value: version, label: version });
  }

  // a pack is published for the loaders it runs on; Paper-family software takes plugins instead
  const modpackLoader: Loader | null = choosesSoftware ? (onboarding.modpackLoaders[variant ?? ''] ?? null) : null;
  const offersModpacks =
    canModpack && !!modpacks.data?.loaders.length && (choosesSoftware ? modpackLoader !== null : true);
  const pack = offersModpacks ? project : null;

  const bedrockPossible =
    canBedrock &&
    (pack
      ? modpackLoader
        ? onboarding.bedrockLoaders.includes(modpackLoader)
        : (modpacks.data?.loaders ?? []).some((loader) => onboarding.bedrockLoaders.includes(loader))
      : choosesSoftware
        ? onboarding.bedrockVariants.includes(variant ?? '')
        : onboarding.bedrockNow);

  const nameError = (names: string[]) => {
    const invalid = invalidNames(names);
    if (invalid.length > 0) return t(`${base}.players.invalid`, { names: invalid.join(', ') });
    if (names.length > MAX_NAMES) return t(`${base}.players.tooMany`, { max: MAX_NAMES });
    return undefined;
  };
  const playersError = privateServer ? nameError(players) : undefined;
  const operatorsError = nameError(operators);

  const chooseVariant = (value: string) => {
    if (value === variant) return;
    // each loader lists other versions, and a pack is made for one loader
    setVariant(value);
    if (choosesVersion) setVersion(LATEST);
    setProject(null);
    setModpackVersion(null);
  };

  const submit = () => {
    if (playersError || operatorsError) return;
    setLoading(true);

    const current = software.data;
    const changesSoftware =
      !!pack ||
      (choosesSoftware && !!variant && variant !== current?.current) ||
      (choosesVersion && !!version && version !== current?.version);

    completeMinecraftOnboarding(server.uuid, {
      variant: !pack && choosesSoftware && variant && variant !== current?.current ? variant : null,
      version: !pack && choosesVersion && version && version !== current?.version ? version : null,
      modpack: pack && modpackVersion ? { source: pack.source, project: pack.id, version: modpackVersion } : null,
      bedrock: bedrock && bedrockPossible,
      onlineMode,
      private: privateServer,
      players: privateServer ? players : [],
      operators,
    })
      .then(() => {
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.onboarding() });
        addToast(
          // the Pelican panel's closing notification: a reinstall applies everything by itself,
          // a server that is up reads the files again only when it restarts
          server.status === 'installing' || changesSoftware
            ? t(`${base}.toast.doneInstalling`, {})
            : powerState !== 'offline'
              ? t(`${base}.toast.doneRestart`, {})
              : t(`${base}.toast.done`, {}),
          'success',
        );
        navigate(`/server/${server.uuidShort}`);
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
        setLoading(false);
      });
  };

  if ((canSoftware && software.isLoading) || (canModpack && modpacks.isLoading)) {
    return <Spinner.Centered />;
  }

  const steps: WizardStep[] = [];

  if (canSoftware && software.data && (choosesSoftware || choosesVersion)) {
    steps.push({
      key: 'software',
      title: t(`${base}.steps.software.title`, {}),
      description: t(`${base}.steps.software.description`, {}),
      content: (
        <>
          {choosesSoftware && (
            <SetupCards label={t(`${base}.software.label`, {})}>
              {software.data.variants.map((choice) => {
                const description = softwareDescription(choice.value);

                return (
                  <SetupCard
                    key={choice.value}
                    selected={choice.value === variant}
                    label={choice.label}
                    description={description ? t(SOFTWARE_DESCRIPTIONS[description], {}) : undefined}
                    onSelect={() => chooseVariant(choice.value)}
                  />
                );
              })}
            </SetupCards>
          )}
          {choosesVersion && (
            <Select
              label={t(`${base}.software.version`, {})}
              description={
                versions.isError
                  ? t('pages.server.settings.software.versionsFailed', {})
                  : t(`${base}.software.versionHint`, {})
              }
              inputWrapperOrder={HELPER_BELOW}
              data={versionData}
              value={version}
              onChange={setVersion}
              searchable
            />
          )}
        </>
      ),
    });
  }

  if (offersModpacks) {
    steps.push({
      key: 'modpack',
      title: t(`${base}.steps.modpack.title`, {}),
      description: t(`${base}.steps.modpack.description`, {}),
      validate: () => !project || !!modpackVersion,
      content: (
        <>
          <ModpackSelect
            loader={modpackLoader}
            project={project}
            onChange={(chosen) => {
              setProject(chosen);
              setModpackVersion(null);
            }}
          />
          {project && <ModpackVersionSelect project={project} value={modpackVersion} onChange={setModpackVersion} />}
        </>
      ),
    });
  }

  if (bedrockPossible) {
    steps.push({
      key: 'bedrock',
      title: t(`${base}.steps.bedrock.title`, {}),
      description: t(`${base}.steps.bedrock.description`, {}),
      content: (
        <>
          <SetupCards>
            <SetupCard
              selected={bedrock}
              label={t(`${base}.bedrock.yes`, {})}
              description={t(`${base}.bedrock.yesDescription`, {})}
              onSelect={() => setBedrock(true)}
            />
            <SetupCard
              selected={!bedrock}
              label={t(`${base}.bedrock.no`, {})}
              description={t(`${base}.bedrock.noDescription`, {})}
              onSelect={() => setBedrock(false)}
            />
          </SetupCards>
          {bedrock && <SetupNote>{t(`${base}.bedrock.note`, {})}</SetupNote>}
        </>
      ),
    });
  }

  steps.push(
    {
      key: 'accounts',
      title: t(`${base}.steps.accounts.title`, {}),
      description: t(`${base}.steps.accounts.description`, {}),
      content: (
        <>
          <SetupCards>
            <SetupCard
              selected={onlineMode}
              label={t(`${base}.accounts.premium`, {})}
              description={t(`${base}.accounts.premiumDescription`, {})}
              onSelect={() => setOnlineMode(true)}
            />
            <SetupCard
              selected={!onlineMode}
              label={t(`${base}.accounts.cracked`, {})}
              description={t(`${base}.accounts.crackedDescription`, {})}
              onSelect={() => setOnlineMode(false)}
            />
          </SetupCards>
          {!onlineMode && <SetupNote>{t(`${base}.accounts.crackedWarning`, {})}</SetupNote>}
        </>
      ),
    },
    {
      key: 'access',
      title: t(`${base}.steps.access.title`, {}),
      description: t(`${base}.steps.access.description`, {}),
      content: (
        <>
          <SetupCards>
            <SetupCard
              selected={privateServer}
              label={t(`${base}.access.private`, {})}
              description={t(`${base}.access.privateDescription`, {})}
              onSelect={() => setPrivateServer(true)}
            />
            <SetupCard
              selected={!privateServer}
              label={t(`${base}.access.public`, {})}
              description={t(`${base}.access.publicDescription`, {})}
              onSelect={() => setPrivateServer(false)}
            />
          </SetupCards>
          {!privateServer && <SetupNote>{t(`${base}.access.publicWarning`, {})}</SetupNote>}
        </>
      ),
    },
    {
      key: 'players',
      title: t(`${base}.steps.players.title`, {}),
      description: t(`${base}.steps.players.description`, {}),
      validate: () => !playersError && !operatorsError,
      content: (
        <>
          {privateServer && (
            <TagsInput
              label={t(`${base}.players.whitelist`, {})}
              description={t(`${base}.players.whitelistHint`, {})}
              placeholder={t(`${base}.players.placeholder`, {})}
              value={players}
              onChange={setPlayers}
              invalidTags={invalidNames(players)}
              error={playersError}
              allowReordering={false}
            />
          )}
          <TagsInput
            label={t(`${base}.players.operators`, {})}
            description={t(`${base}.players.operatorsHint`, {})}
            placeholder={t(`${base}.players.placeholder`, {})}
            value={operators}
            onChange={setOperators}
            invalidTags={invalidNames(operators)}
            error={operatorsError}
            allowReordering={false}
          />
          <SetupNote>{t(`${base}.players.operatorsWarning`, {})}</SetupNote>
        </>
      ),
    },
  );

  return (
    <Wizard
      steps={steps}
      active={active}
      onActiveChange={setActive}
      submitLabel={t(`${base}.finish`, {})}
      submitting={loading}
      onSubmit={submit}
    />
  );
}

export default function ServerMinecraftSetup() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const setup = useMinecraftSetup();

  // once answered, how applying it goes is shown on the console (SetupCallout)
  if (!setup.isLoading && !setup.data?.pending) {
    return <Navigate to={`/server/${server.uuidShort}`} replace />;
  }

  return (
    <ServerContentContainer title={t(`${base}.title`, {})} subtitle={t(`${base}.subtitle`, {})}>
      {setup.data ? <SetupForm onboarding={setup.data} /> : <Spinner.Centered />}
    </ServerContentContainer>
  );
}
