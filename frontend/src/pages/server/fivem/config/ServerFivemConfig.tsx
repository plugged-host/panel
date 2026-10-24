// plugged.host: the server.cfg txAdmin runs, as a form; only what changed is written (docs/features/fivem.md)
import { faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery } from '@tanstack/react-query';
import { ReactNode, useEffect, useState } from 'react';
import { createSearchParams, useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getDatabases from '@/api/server/databases/getDatabases.ts';
import getFivemConfig from '@/api/server/fivem/getFivemConfig.ts';
import updateFivemConfig from '@/api/server/fivem/updateFivemConfig.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import PasswordInput from '@/elements/input/PasswordInput.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TagsInput from '@/elements/input/TagsInput.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import SegmentedControl from '@/elements/layout/SegmentedControl.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Anchor from '@/elements/typography/Anchor.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  fivemAceSchema,
  fivemConfigSchema,
  fivemPrincipalSchema,
  fivemResourceSchema,
  fivemSettingsSchema,
} from '@/lib/schemas/server/fivem.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Config = z.infer<typeof fivemConfigSchema>;
type Settings = z.infer<typeof fivemSettingsSchema>;
type Resource = z.infer<typeof fivemResourceSchema>;
type Principal = z.infer<typeof fivemPrincipalSchema>;
type Ace = z.infer<typeof fivemAceSchema>;

const base = 'pages.server.fivem.config';
/** Rows a long list shows until "Show all". */
const FOLD = 25;
const NAME = /^[\p{L}\p{N}_\-.@[\]]+$/u;
const PRINCIPAL = /^[\p{L}\p{N}_\-.:@[\]]+$/u;

/** GTA V game builds (sv_enforceGameBuild) and the update each brought. */
const FIVEM_BUILDS: [number, string][] = [
  [3751, 'A Safehouse in the Hills'],
  [3570, 'Money Fronts'],
  [3407, 'Agents of Sabotage'],
  [3323, 'Bottom Dollar Bounties (patch)'],
  [3258, 'Bottom Dollar Bounties'],
  [3095, 'The Chop Shop'],
  [2944, 'San Andreas Mercenaries'],
  [2802, 'Los Santos Drug Wars'],
  [2699, 'The Criminal Enterprises'],
  [2612, 'The Contract (patch)'],
  [2545, 'The Contract'],
  [2372, 'Los Santos Tuners'],
  [2189, 'Cayo Perico Heist'],
  [2060, 'Los Santos Summer Special'],
  [1604, 'Base game'],
];
const REDM_BUILDS = [1491, 1436, 1355, 1311];

const LOCALES = [
  'en-US',
  'en-GB',
  'de-DE',
  'fr-FR',
  'es-ES',
  'es-MX',
  'pt-BR',
  'pt-PT',
  'it-IT',
  'nl-NL',
  'pl-PL',
  'cs-CZ',
  'sk-SK',
  'hu-HU',
  'ro-RO',
  'bg-BG',
  'el-GR',
  'tr-TR',
  'ru-RU',
  'uk-UA',
  'sv-SE',
  'da-DK',
  'nb-NO',
  'fi-FI',
  'ar-SA',
  'he-IL',
  'hi-IN',
  'id-ID',
  'th-TH',
  'vi-VN',
  'zh-CN',
  'zh-TW',
  'ja-JP',
  'ko-KR',
];

function Section({ title, description, children }: { title: string; description: string; children: ReactNode }) {
  return (
    <Stack gap='sm'>
      <Stack gap={2}>
        <Title order={3} fz='md' fw={600}>
          {title}
        </Title>
        <Text size='sm' c='dimmed'>
          {description}
        </Text>
      </Stack>
      {children}
    </Stack>
  );
}

function RowList<T>({
  title,
  description,
  rows,
  setRows,
  blank,
  disabled,
  render,
}: {
  title: string;
  description: string;
  rows: T[];
  setRows: (rows: T[]) => void;
  blank: T;
  disabled: boolean;
  render: (row: T, update: (row: T) => void) => ReactNode;
}) {
  const { t } = useTranslations();
  const [all, setAll] = useState(false);
  const shown = all ? rows : rows.slice(0, FOLD);

  return (
    <Section title={title} description={description}>
      {rows.length === 0 && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.lists.empty`, {})}
        </Text>
      )}
      {shown.map((row, index) => (
        // rows have no stable id of their own (new ones have no line yet): the position is it
        <Group key={index} gap='xs' wrap='nowrap' align='flex-end'>
          {render(row, (changed) => setRows(rows.map((old, at) => (at === index ? changed : old))))}
          {!disabled && (
            <Tooltip label={t(`${base}.lists.remove`, {})}>
              <ActionIcon
                variant='default'
                size='input-sm'
                aria-label={t(`${base}.lists.remove`, {})}
                onClick={() => setRows(rows.filter((_, at) => at !== index))}
              >
                <FontAwesomeIcon icon={faTrash} />
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
      ))}
      <Group gap='sm'>
        {!disabled && (
          <Button
            variant='default'
            size='xs'
            onClick={() => {
              setRows([...rows, blank]);
              setAll(true);
            }}
          >
            {t(`${base}.lists.add`, {})}
          </Button>
        )}
        {!all && rows.length > FOLD && (
          <Button variant='subtle' size='xs' onClick={() => setAll(true)}>
            {t(`${base}.lists.showAll`, { count: rows.length })}
          </Button>
        )}
      </Group>
    </Section>
  );
}

function ConfigForm({ config, onSaved }: { config: Config; onSaved: (config: Config) => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const canEditText = useServerCan(['files.read-content', 'files.update'], false);
  const canReadDatabases = useServerCan('databases.read');

  const [settings, setSettings] = useState<Settings>(config.settings);
  const [resources, setResources] = useState<Resource[]>(config.resources);
  const [principals, setPrincipals] = useState<Principal[]>(config.principals);
  const [aces, setAces] = useState<Ace[]>(config.aces);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setSettings(config.settings);
    setResources(config.resources);
    setPrincipals(config.principals);
    setAces(config.aces);
  }, [config]);

  const databases = useQuery({
    queryKey: [...queryKeys.server(server.uuid).fivem.config(), 'databases'],
    queryFn: () => getDatabases(server.uuid, 1, undefined, 100),
    enabled: canReadDatabases,
  });
  const mysql = (databases.data?.data ?? []).filter((database) => database.type === 'mysql');

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setSettings((settings) => ({ ...settings, [key]: value }));
  const setDatabase = (values: Partial<Settings['database']>) =>
    setSettings((settings) => ({ ...settings, database: { ...settings.database, ...values } }));

  const invalid =
    resources.some((row) => !NAME.test(row.name.trim())) ||
    principals.some((row) => !PRINCIPAL.test(row.child.trim()) || !PRINCIPAL.test(row.parent.trim())) ||
    aces.some((row) => !PRINCIPAL.test(row.principal.trim()) || !PRINCIPAL.test(row.object.trim()));

  const save = () => {
    if (!config.hash) return;
    setSaving(true);

    updateFivemConfig(server.uuid, { hash: config.hash, settings, resources, principals, aces })
      .then((saved) => {
        addToast(t(`${base}.toast.saved`, {}), 'success');
        onSaved(saved);
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSaving(false));
  };

  const path = config.path ?? '';
  const slash = path.lastIndexOf('/');
  const editorUrl = `/server/${server.uuidShort}/files/edit?${createSearchParams({
    directory: slash === -1 ? '/' : `/${path.slice(0, slash)}`,
    file: path.slice(slash + 1),
  })}`;

  const gameBuilds = config.redm
    ? REDM_BUILDS.map((build) => ({ value: String(build), label: String(build) }))
    : FIVEM_BUILDS.map(([build, name]) => ({ value: String(build), label: `${build} · ${name}` }));
  if (settings.gameBuild && !gameBuilds.some((build) => build.value === settings.gameBuild)) {
    gameBuilds.unshift({ value: settings.gameBuild, label: settings.gameBuild });
  }
  const locales = [
    { value: 'root-AQ', label: t(`${base}.locale.none`, {}) },
    ...LOCALES.map((l) => ({ value: l, label: l })),
  ];
  if (settings.locale && !locales.some((locale) => locale.value === settings.locale)) {
    locales.push({ value: settings.locale, label: settings.locale });
  }

  return (
    <Stack gap='xl'>
      <Text size='sm' c='dimmed'>
        {t(`${base}.intro`, { file: `/${path}` })}
      </Text>

      <Section title={t(`${base}.identity.title`, {})} description={t(`${base}.identity.description`, {})}>
        <div className='grid gap-4 md:grid-cols-2'>
          <TextInput
            label={t(`${base}.identity.hostname`, {})}
            description={t(`${base}.identity.hostnameDescription`, {})}
            value={settings.hostname}
            onChange={(e) => set('hostname', e.currentTarget.value)}
            maxLength={255}
            className='md:col-span-2'
          />
          <TextInput
            label={t(`${base}.identity.projectName`, {})}
            description={t(`${base}.identity.projectNameDescription`, {})}
            value={settings.projectName}
            onChange={(e) => set('projectName', e.currentTarget.value)}
            maxLength={255}
          />
          <TextInput
            label={t(`${base}.identity.maxClients`, {})}
            description={t(`${base}.identity.maxClientsDescription`, {})}
            value={settings.maxClients}
            onChange={(e) => set('maxClients', e.currentTarget.value.replace(/\D/g, ''))}
            inputMode='numeric'
            maxLength={4}
          />
          <TextInput
            label={t(`${base}.identity.projectDescription`, {})}
            description={t(`${base}.identity.projectDescriptionDescription`, {})}
            value={settings.projectDescription}
            onChange={(e) => set('projectDescription', e.currentTarget.value)}
            maxLength={255}
          />
          <Select
            label={t(`${base}.identity.locale`, {})}
            description={t(`${base}.identity.localeDescription`, {})}
            data={locales}
            value={settings.locale || null}
            onChange={(value) => set('locale', value ?? '')}
            searchable
          />
          <div className='md:col-span-2'>
            <TagsInput
              label={t(`${base}.identity.tags`, {})}
              description={t(`${base}.identity.tagsDescription`, {})}
              placeholder={t(`${base}.identity.tagsPlaceholder`, {})}
              value={settings.tags}
              onChange={(tags) => set('tags', tags)}
              allowReordering={false}
            />
          </div>
          <TextInput
            label={t(`${base}.identity.icon`, {})}
            description={t(`${base}.identity.iconDescription`, {})}
            placeholder='myLogo.png'
            value={settings.icon}
            onChange={(e) => set('icon', e.currentTarget.value)}
            maxLength={255}
            className='md:col-span-2'
          />
        </div>
      </Section>

      <Section title={t(`${base}.network.title`, {})} description={t(`${base}.network.description`, {})}>
        <div className='grid gap-4 md:grid-cols-2'>
          <Stack gap={2}>
            <Text size='sm' fw={500}>
              {t(`${base}.network.endpoints`, {})}
            </Text>
            <Text size='sm'>
              {config.endpoints.length > 0 ? config.endpoints.join(', ') : t(`${base}.network.noEndpoints`, {})}
            </Text>
            <Text size='xs' c='dimmed'>
              {t(`${base}.network.endpointsDescription`, {})}
            </Text>
          </Stack>
          <Stack gap={2}>
            <Text size='sm' fw={500}>
              {t(`${base}.network.onesync`, {})}
            </Text>
            <Text size='sm'>{t(`${base}.network.onesyncValue`, {})}</Text>
            <Text size='xs' c='dimmed'>
              {t(`${base}.network.onesyncDescription`, {})}
            </Text>
          </Stack>
          <Select
            label={t(`${base}.network.gameBuild`, {})}
            description={t(`${base}.network.gameBuildDescription`, {})}
            placeholder={t(`${base}.network.gameBuildNone`, {})}
            data={gameBuilds}
            value={settings.gameBuild || null}
            onChange={(value) => set('gameBuild', value ?? '')}
            clearable
            className='md:col-span-2'
          />
        </div>
      </Section>

      <Section title={t(`${base}.database.title`, {})} description={t(`${base}.database.description`, {})}>
        {mysql.length > 0 && (
          <Select
            label={t(`${base}.database.use`, {})}
            description={t(`${base}.database.useDescription`, {})}
            placeholder={t(`${base}.database.usePlaceholder`, {})}
            data={mysql.map((database) => ({ value: database.uuid, label: `${database.name} (${database.host})` }))}
            value={null}
            onChange={(uuid) => {
              const database = mysql.find((database) => database.uuid === uuid);
              if (!database) return;
              setDatabase({
                host: database.host,
                port: String(database.port),
                user: database.username,
                name: database.name,
                ...(database.password ? { password: database.password } : {}),
              });
            }}
            className='max-w-md'
          />
        )}
        <div className='grid gap-4 md:grid-cols-3'>
          <TextInput
            label={t(`${base}.database.host`, {})}
            placeholder='localhost'
            value={settings.database.host}
            onChange={(e) => setDatabase({ host: e.currentTarget.value })}
            maxLength={255}
            className='md:col-span-2'
          />
          <TextInput
            label={t(`${base}.database.port`, {})}
            placeholder='3306'
            value={settings.database.port}
            onChange={(e) => setDatabase({ port: e.currentTarget.value.replace(/\D/g, '') })}
            inputMode='numeric'
            maxLength={5}
          />
          <TextInput
            label={t(`${base}.database.user`, {})}
            value={settings.database.user}
            onChange={(e) => setDatabase({ user: e.currentTarget.value })}
            maxLength={255}
          />
          <TextInput
            label={t(`${base}.database.name`, {})}
            value={settings.database.name}
            onChange={(e) => setDatabase({ name: e.currentTarget.value })}
            maxLength={255}
          />
          <PasswordInput
            label={t(`${base}.database.password`, {})}
            value={settings.database.password}
            onChange={(e) => setDatabase({ password: e.currentTarget.value })}
            maxLength={255}
            autoComplete='new-password'
          />
        </div>
        <Stack gap={4}>
          <Text size='sm' fw={500}>
            {t(`${base}.database.format`, {})}
          </Text>
          <SegmentedControl
            data={[
              { value: 'uri', label: 'URI' },
              { value: 'kv', label: 'key=value' },
            ]}
            value={settings.database.keyValue ? 'kv' : 'uri'}
            onChange={(value) => setDatabase({ keyValue: value === 'kv' })}
            className='self-start'
          />
          <Text size='xs' c='dimmed'>
            {t(`${base}.database.formatDescription`, {})}
          </Text>
        </Stack>
      </Section>

      <Section title={t(`${base}.keys.title`, {})} description={t(`${base}.keys.description`, {})}>
        <div className='grid gap-4 md:grid-cols-2'>
          <PasswordInput
            label={t(`${base}.keys.license`, {})}
            description={
              <>
                {t(`${base}.keys.licenseDescription`, {})}{' '}
                <Anchor href='https://portal.cfx.re/servers/registration-keys' target='_blank' rel='noreferrer'>
                  portal.cfx.re
                </Anchor>
              </>
            }
            value={settings.licenseKey}
            onChange={(e) => set('licenseKey', e.currentTarget.value)}
            maxLength={255}
            autoComplete='new-password'
          />
          <PasswordInput
            label={t(`${base}.keys.steam`, {})}
            description={t(`${base}.keys.steamDescription`, {})}
            value={settings.steamKey}
            onChange={(e) => set('steamKey', e.currentTarget.value)}
            maxLength={255}
            autoComplete='new-password'
          />
          <PasswordInput
            label={t(`${base}.keys.tebex`, {})}
            description={t(`${base}.keys.tebexDescription`, {})}
            value={settings.tebexSecret}
            onChange={(e) => set('tebexSecret', e.currentTarget.value)}
            maxLength={255}
            autoComplete='new-password'
          />
        </div>
      </Section>

      <RowList
        title={t(`${base}.resources.title`, {})}
        description={t(`${base}.resources.description`, {})}
        rows={resources}
        setRows={setResources}
        blank={{ key: null, name: '', enabled: true }}
        disabled={saving}
        render={(row, update) => (
          <>
            <TextInput
              aria-label={t(`${base}.resources.name`, {})}
              placeholder={t(`${base}.resources.name`, {})}
              value={row.name}
              onChange={(e) => update({ ...row, name: e.currentTarget.value })}
              error={row.name.trim() !== '' && !NAME.test(row.name.trim())}
              maxLength={255}
              className='flex-1'
            />
            <Switch
              aria-label={t(`${base}.resources.enabled`, {})}
              checked={row.enabled}
              onChange={(e) => update({ ...row, enabled: e.currentTarget.checked })}
              className='mb-2'
            />
          </>
        )}
      />

      <RowList
        title={t(`${base}.principals.title`, {})}
        description={t(`${base}.principals.description`, {})}
        rows={principals}
        setRows={setPrincipals}
        blank={{ key: null, child: '', parent: '' }}
        disabled={saving}
        render={(row, update) => (
          <>
            <TextInput
              aria-label={t(`${base}.principals.child`, {})}
              placeholder={t(`${base}.principals.childPlaceholder`, {})}
              value={row.child}
              onChange={(e) => update({ ...row, child: e.currentTarget.value })}
              maxLength={255}
              className='flex-1'
            />
            <TextInput
              aria-label={t(`${base}.principals.parent`, {})}
              placeholder='group.admin'
              value={row.parent}
              onChange={(e) => update({ ...row, parent: e.currentTarget.value })}
              maxLength={255}
              className='flex-1'
            />
          </>
        )}
      />

      <RowList
        title={t(`${base}.aces.title`, {})}
        description={t(`${base}.aces.description`, {})}
        rows={aces}
        setRows={setAces}
        blank={{ key: null, principal: '', object: '', access: 'allow' }}
        disabled={saving}
        render={(row, update) => (
          <>
            <TextInput
              aria-label={t(`${base}.aces.principal`, {})}
              placeholder='group.admin'
              value={row.principal}
              onChange={(e) => update({ ...row, principal: e.currentTarget.value })}
              maxLength={255}
              className='flex-1'
            />
            <TextInput
              aria-label={t(`${base}.aces.object`, {})}
              placeholder='command'
              value={row.object}
              onChange={(e) => update({ ...row, object: e.currentTarget.value })}
              maxLength={255}
              className='flex-1'
            />
            <Select
              aria-label={t(`${base}.aces.access`, {})}
              data={[
                { value: 'allow', label: t(`${base}.aces.allow`, {}) },
                { value: 'deny', label: t(`${base}.aces.deny`, {}) },
              ]}
              value={row.access}
              onChange={(value) => update({ ...row, access: value === 'deny' ? 'deny' : 'allow' })}
              allowDeselect={false}
              className='w-28'
            />
          </>
        )}
      />

      {invalid && (
        <Text size='sm' c='red'>
          {t(`${base}.invalid`, {})}
        </Text>
      )}

      <Group>
        <Button onClick={save} loading={saving} disabled={invalid || !config.hash}>
          {t('common.button.save', {})}
        </Button>
        {canEditText && (
          <Button variant='default' onClick={() => navigate(editorUrl)}>
            {t(`${base}.button.editText`, {})}
          </Button>
        )}
      </Group>
    </Stack>
  );
}

export default function ServerFivemConfig() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const config = useResource({
    queryKey: queryKeys.server(server.uuid).fivem.config(),
    queryFn: () => getFivemConfig(server.uuid),
  });

  return (
    <ServerContentContainer title={t(`${base}.title`, {})}>
      <ResourceView resource={config}>
        {(data) =>
          data.path === null ? (
            <Text c='dimmed'>{t(`${base}.notSetUp`, {})}</Text>
          ) : (
            <ConfigForm config={data} onSaved={() => config.refetch()} />
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
