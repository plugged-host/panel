// plugged.host: the setup a player's new Minecraft server starts with, the Pelican panel's setup
// wizard (ServerOnboardingPage, docs/features/minecraft.md)
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ReactNode, useEffect, useRef, useState } from 'react';
import { Navigate, useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import chooseMinecraftSoftware, { MinecraftSoftwarePick } from '@/api/server/minecraft/chooseMinecraftSoftware.ts';
import completeMinecraftOnboarding from '@/api/server/minecraft/completeMinecraftOnboarding.ts';
import getServerSoftware from '@/api/server/settings/getServerSoftware.ts';
import getSoftwareVersions from '@/api/server/settings/getSoftwareVersions.ts';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Choice from '@/elements/input/Choice.tsx';
import Select from '@/elements/input/Select.tsx';
import TagsInput from '@/elements/input/TagsInput.tsx';
import Text from '@/elements/typography/Text.tsx';
import Wizard, { WizardStep } from '@/elements/Wizard.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  minecraftOnboardingSchema,
  minecraftProjectSchema,
  minecraftVersionSchema,
} from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useModpacks } from '../content/useModpacks.ts';
import ModpackPicker from './ModpackPicker.tsx';
import SetupProgress from './SetupProgress.tsx';
import { setupRunning, useMinecraftSetup } from './useMinecraftSetup.ts';

type Onboarding = z.infer<typeof minecraftOnboardingSchema>;
type Project = z.infer<typeof minecraftProjectSchema>;
type Version = z.infer<typeof minecraftVersionSchema>;

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
  // a modpack is a choice next to the software: it brings its own loader and version
  const [usePack, setUsePack] = useState(false);
  const [project, setProject] = useState<Project | null>(null);
  const [packVersion, setPackVersion] = useState<Version | null>(null);
  const modpackVersion = packVersion?.id ?? null;
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

  // packs exist when the server's game can run a mod loader
  const offersModpacks = canModpack && !!modpacks.data?.loaders.length;
  const pack = offersModpacks && usePack ? project : null;

  // the pack's own loaders decide, else the software's
  const bedrockPossible =
    canBedrock &&
    (pack
      ? (packVersion?.loaders ?? []).some((loader) =>
          onboarding.bedrockLoaders.some((bedrockLoader) => bedrockLoader === loader.toLowerCase()),
        )
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

  // what the server runs is put in place as soon as its step is left, while the rest is answered
  const softwarePick = (withModpack: boolean): MinecraftSoftwarePick => ({
    variant: choosesSoftware ? variant : null,
    version: choosesVersion ? version : null,
    modpack:
      withModpack && pack && modpackVersion ? { source: pack.source, project: pack.id, version: modpackVersion } : null,
  });
  const sentPick = useRef<string | null>(null);
  const choose = (pick: MinecraftSoftwarePick) => {
    const key = JSON.stringify(pick);
    if (key === sentPick.current) return;
    // the software it was created with needs nothing, until another pick changed it
    const unchanged =
      !pick.modpack &&
      (pick.variant ?? software.data?.current) === software.data?.current &&
      (pick.version ?? software.data?.version) === software.data?.version;
    if (sentPick.current === null && unchanged) return;
    sentPick.current = key;

    chooseMinecraftSoftware(server.uuid, pick)
      .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.onboarding() }))
      .catch((error) => {
        sentPick.current = null;
        addToast(httpErrorToHuman(error), 'error');
      });
  };

  const chooseVariant = (value: string) => {
    setUsePack(false);
    if (value === variant) return;
    // each loader lists other versions
    setVariant(value);
    if (choosesVersion) setVersion(LATEST);
  };

  // a pack goes on the server the moment it is clicked (its newest version, changeable)
  const pickPack = (chosen: Project, chosenVersion: Version) => {
    setProject(chosen);
    setPackVersion(chosenVersion);
    choose({
      variant: null,
      version: null,
      modpack: { source: chosen.source, project: chosen.id, version: chosenVersion.id },
    });
  };

  const submit = () => {
    if (playersError || operatorsError) return;
    setLoading(true);

    const current = software.data;
    const changesSoftware =
      !!pack ||
      (choosesSoftware && !!variant && variant !== current?.current) ||
      (choosesVersion && !!version && version !== current?.version);

    // as picked, not as changed: an earlier pick may have changed the server already
    completeMinecraftOnboarding(server.uuid, {
      variant: !pack && choosesSoftware && variant ? variant : null,
      version: !pack && choosesVersion && version ? version : null,
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

  if (canSoftware && software.data && (choosesSoftware || choosesVersion || offersModpacks)) {
    steps.push({
      key: 'software',
      title: t(`${base}.steps.software.title`, {}),
      description: t(`${base}.steps.software.description`, {}),
      // the Modpack card wants a pack picked
      validate: () => !(usePack && offersModpacks) || (!!project && !!modpackVersion),
      content: (
        <>
          {(choosesSoftware || offersModpacks) && (
            <SetupCards label={t(`${base}.software.label`, {})}>
              {choosesSoftware ? (
                software.data.variants.map((choice) => {
                  const description = softwareDescription(choice.value);

                  return (
                    <SetupCard
                      key={choice.value}
                      selected={!usePack && choice.value === variant}
                      label={choice.label}
                      description={description ? t(SOFTWARE_DESCRIPTIONS[description], {}) : undefined}
                      onSelect={() => chooseVariant(choice.value)}
                    />
                  );
                })
              ) : (
                <SetupCard
                  selected={!usePack}
                  label={t(`${base}.modpack.none`, {})}
                  onSelect={() => setUsePack(false)}
                />
              )}
              {offersModpacks && (
                <SetupCard
                  selected={usePack}
                  label={t(`${base}.modpack.label`, {})}
                  description={t(`${base}.modpack.card`, {})}
                  onSelect={() => setUsePack(true)}
                />
              )}
            </SetupCards>
          )}
          {usePack && offersModpacks && (
            <ModpackPicker
              sources={modpacks.data?.sources ?? ['modrinth']}
              project={project}
              version={packVersion}
              onPick={pickPack}
              onClear={() => {
                setProject(null);
                setPackVersion(null);
              }}
            />
          )}
          {!usePack && choosesVersion && (
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

  const changeStep = (next: number) => {
    const left = steps[active]?.key;
    // a pack was sent when it was clicked; the software goes with the step's Next
    if (next > active && canSoftware && left === 'software') choose(softwarePick(usePack));
    setActive(next);
  };

  return (
    <Wizard
      steps={steps}
      active={active}
      onActiveChange={changeStep}
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

  // the picked software being put in place while the rest is answered
  const working = setupRunning(setup.data?.progress);

  return (
    <ServerContentContainer title={t(`${base}.title`, {})} subtitle={t(`${base}.subtitle`, {})}>
      {setup.data && working && (
        <Notification
          className='theme-callout mb-6'
          loading={setup.data.progress?.step !== 'ready'}
          title={t(`${base}.progress.title`, {})}
        >
          <SetupProgress onboarding={setup.data} />
        </Notification>
      )}
      {setup.data ? <SetupForm onboarding={setup.data} /> : <Spinner.Centered />}
    </ServerContentContainer>
  );
}
