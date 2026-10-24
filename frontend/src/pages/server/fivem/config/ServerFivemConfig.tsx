// plugged.host: the server.cfg txAdmin runs, as a form laid out like the Pelican panel's Server configuration page;
// only what changed is written (docs/features/fivem.md)
import { faArrowsRotate, faXmark } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { TagsInput } from '@mantine/core';
import { useQuery } from '@tanstack/react-query';
import { ReactNode, useEffect, useState } from 'react';
import { NavLink, useNavigate } from 'react-router';
import { z } from 'zod';
import { getHttpStatus, httpErrorToHuman } from '@/api/axios.ts';
import getDatabases from '@/api/server/databases/getDatabases.ts';
import getFileContent from '@/api/server/files/getFileContent.ts';
import saveFileContent from '@/api/server/files/saveFileContent.ts';
import getFivemConfig from '@/api/server/fivem/getFivemConfig.ts';
import updateFivemConfig from '@/api/server/fivem/updateFivemConfig.ts';
import sendPowerAction from '@/api/server/sendPowerAction.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import AsideSection from '@/elements/data-display/AsideSection.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import MonacoEditor from '@/elements/editors/MonacoEditor.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import PasswordInput from '@/elements/input/PasswordInput.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Anchor from '@/elements/typography/Anchor.tsx';
import { formatAllocation } from '@/lib/domain/server.ts';
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
type List = 'resources' | 'principals' | 'aces';

const base = 'pages.server.fivem.config';
/** Rows a long list shows until "Show all". */
const FOLD = 25;
const NAME = /^[\p{L}\p{N}_\-.@[\]]+$/u;
const PRINCIPAL = /^[\p{L}\p{N}_\-.:@[\]]+$/u;
/** Filament's helper text sits under the field, not between label and field. */
const BELOW: ('label' | 'input' | 'description' | 'error')[] = ['label', 'input', 'description', 'error'];

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

/** The first row a save would refuse (its list and its number from 1), checked the way the Pelican page does. */
function firstInvalidRow(resources: Resource[], principals: Principal[], aces: Ace[]): [List, number] | null {
  const lists: [List, RegExp, string[][]][] = [
    ['resources', NAME, resources.map((row) => [row.name])],
    ['principals', PRINCIPAL, principals.map((row) => [row.child, row.parent])],
    ['aces', PRINCIPAL, aces.map((row) => [row.principal, row.object])],
  ];

  for (const [list, pattern, rows] of lists) {
    const index = rows.findIndex((values) => values.some((value) => !pattern.test(value.trim())));
    if (index !== -1) return [list, index + 1];
  }

  return null;
}

/** A field the form shows but does not edit (Filament's TextEntry). */
function Entry({ label, value, hint }: { label: string; value: ReactNode; hint: string }) {
  return (
    <div className='theme-fivem-entry'>
      <span className='theme-fivem-entry-label'>{label}</span>
      <span className='theme-fivem-entry-value'>{value}</span>
      <span className='theme-fivem-entry-hint'>{hint}</span>
    </div>
  );
}

function RowList<T>({
  list,
  rows,
  setRows,
  blank,
  columns,
  render,
}: {
  list: List;
  rows: T[];
  setRows: (rows: T[]) => void;
  blank: T;
  columns: string[];
  render: (row: T, update: (row: T) => void) => ReactNode;
}) {
  const { t } = useTranslations();
  const [all, setAll] = useState(false);
  const shown = all ? rows : rows.slice(0, FOLD);
  const remove = t(`${base}.lists.remove`, {});

  return (
    <AsideSection title={t(`${base}.${list}.title`, {})} description={t(`${base}.${list}.description`, {})}>
      <div className='flex flex-col gap-2'>
        {rows.length > 0 && (
          <div className='theme-fivem-line' data-list={list}>
            {columns.map((column) => (
              <span key={column} className='theme-fivem-head'>
                {column}
              </span>
            ))}
            <span />
          </div>
        )}
        {shown.map((row, index) => (
          // rows have no stable id of their own (new ones have no line yet): the position is it
          <div key={index} className='theme-fivem-line' data-list={list}>
            {render(row, (changed) => setRows(rows.map((old, at) => (at === index ? changed : old))))}
            <Tooltip label={remove}>
              <ActionIcon
                className='theme-fivem-x'
                variant='subtle'
                size={32}
                aria-label={remove}
                onClick={() => setRows(rows.filter((_, at) => at !== index))}
              >
                <FontAwesomeIcon icon={faXmark} />
              </ActionIcon>
            </Tooltip>
          </div>
        ))}

        <div className='mt-1 flex flex-wrap items-center gap-3'>
          <Button
            variant='default'
            size='sm'
            onClick={() => {
              setRows([...rows, blank]);
              setAll(true);
            }}
          >
            {t(`${base}.${list}.add`, {})}
          </Button>
          {!all && rows.length > FOLD && (
            <button type='button' className='theme-eco-link' onClick={() => setAll(true)}>
              {t(`${base}.lists.showAll`, { count: rows.length })}
            </button>
          )}
          {rows.length === 0 && <span className='theme-eco-muted'>{t(`${base}.lists.empty`, {})}</span>}
        </div>
      </div>
    </AsideSection>
  );
}

function ConfigForm({ config, onSaved }: { config: Config; onSaved: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
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

  const save = () => {
    if (!config.hash) return;

    const invalid = firstInvalidRow(resources, principals, aces);
    if (invalid) {
      addToast(t(`${base}.invalid`, { list: t(`${base}.${invalid[0]}.title`, {}), row: invalid[1] }), 'error');
      return;
    }

    setSaving(true);
    updateFivemConfig(server.uuid, { hash: config.hash, settings, resources, principals, aces })
      .then((saved) => {
        // a save that writes nothing leaves the file, and so its hash, as it was
        if (saved.hash === config.hash) {
          addToast(t(`${base}.toast.noChanges`, {}), 'info');
        } else {
          addToast(t(`${base}.toast.saved`, {}), 'success');
        }
        onSaved();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSaving(false));
  };

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
  const keyValue = settings.database.keyValue;

  return (
    <div className='flex flex-col gap-6'>
      <AsideSection title={t(`${base}.identity.title`, {})} description={t(`${base}.identity.description`, {})}>
        <div className='grid gap-6 lg:grid-cols-2'>
          <TextInput
            label={t(`${base}.identity.hostname`, {})}
            description={t(`${base}.identity.hostnameDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder=''
            value={settings.hostname}
            onChange={(e) => set('hostname', e.currentTarget.value)}
            maxLength={255}
            className='lg:col-span-2'
          />
          <TextInput
            label={t(`${base}.identity.projectName`, {})}
            description={t(`${base}.identity.projectNameDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder=''
            value={settings.projectName}
            onChange={(e) => set('projectName', e.currentTarget.value)}
            maxLength={255}
          />
          <TextInput
            label={t(`${base}.identity.maxClients`, {})}
            description={t(`${base}.identity.maxClientsDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder=''
            value={settings.maxClients}
            onChange={(e) => set('maxClients', e.currentTarget.value.replace(/\D/g, ''))}
            inputMode='numeric'
            maxLength={4}
          />
          <TextInput
            label={t(`${base}.identity.projectDescription`, {})}
            description={t(`${base}.identity.projectDescriptionDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder=''
            value={settings.projectDescription}
            onChange={(e) => set('projectDescription', e.currentTarget.value)}
            maxLength={255}
          />
          <Select
            label={t(`${base}.identity.locale`, {})}
            description={t(`${base}.identity.localeDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder=''
            data={locales}
            value={settings.locale || null}
            onChange={(value) => set('locale', value ?? '')}
            searchable
          />
          <TagsInput
            label={t(`${base}.identity.tags`, {})}
            description={t(`${base}.identity.tagsDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder={t(`${base}.identity.tagsPlaceholder`, {})}
            value={settings.tags}
            onChange={(tags) => set('tags', tags)}
            splitChars={[',']}
            className='lg:col-span-2'
          />
          <TextInput
            label={t(`${base}.identity.icon`, {})}
            description={t(`${base}.identity.iconDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder='myLogo.png'
            value={settings.icon}
            onChange={(e) => set('icon', e.currentTarget.value)}
            maxLength={255}
            className='lg:col-span-2'
          />
        </div>
      </AsideSection>

      <AsideSection title={t(`${base}.network.title`, {})} description={t(`${base}.network.description`, {})}>
        <div className='grid gap-6 lg:grid-cols-2'>
          <Entry
            label={t(`${base}.network.endpoints`, {})}
            value={
              config.endpoints.length > 0
                ? config.endpoints.join(', ')
                : t(`${base}.network.noEndpoints`, { address: formatAllocation(server.allocation) })
            }
            hint={t(`${base}.network.endpointsDescription`, {})}
          />
          <Entry
            label={t(`${base}.network.onesync`, {})}
            value={t(`${base}.network.onesyncValue`, {})}
            hint={t(`${base}.network.onesyncDescription`, {})}
          />
          <Select
            label={t(`${base}.network.gameBuild`, {})}
            description={t(`${base}.network.gameBuildDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder={t(`${base}.network.gameBuildNone`, {})}
            data={gameBuilds}
            value={settings.gameBuild || null}
            onChange={(value) => set('gameBuild', value ?? '')}
            clearable
            className='lg:col-span-2'
          />
        </div>
      </AsideSection>

      <AsideSection title={t(`${base}.database.title`, {})} description={t(`${base}.database.description`, {})}>
        <div className='grid gap-6 lg:grid-cols-3'>
          {mysql.length > 0 && (
            <Select
              label={t(`${base}.database.use`, {})}
              description={t(`${base}.database.useDescription`, {})}
              inputWrapperOrder={BELOW}
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
              className='lg:col-span-3'
            />
          )}
          <TextInput
            label={t(`${base}.database.host`, {})}
            placeholder='localhost'
            value={settings.database.host}
            onChange={(e) => setDatabase({ host: e.currentTarget.value })}
            maxLength={255}
            className='lg:col-span-2'
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
            placeholder=''
            value={settings.database.user}
            onChange={(e) => setDatabase({ user: e.currentTarget.value })}
            maxLength={255}
          />
          <TextInput
            label={t(`${base}.database.name`, {})}
            placeholder=''
            value={settings.database.name}
            onChange={(e) => setDatabase({ name: e.currentTarget.value })}
            maxLength={255}
          />
          <PasswordInput
            label={t(`${base}.database.password`, {})}
            placeholder=''
            value={settings.database.password}
            onChange={(e) => setDatabase({ password: e.currentTarget.value })}
            maxLength={255}
            autoComplete='new-password'
          />
          {/* Filament's grouped toggle buttons: two pills, the chosen one filled */}
          <div className='theme-fivem-entry lg:col-span-3'>
            <span id='fivem-db-format' className='theme-fivem-entry-label'>
              {t(`${base}.database.format`, {})}
            </span>
            <div className='flex flex-wrap gap-2' role='group' aria-labelledby='fivem-db-format'>
              <Button
                variant={keyValue ? 'default' : 'filled'}
                aria-pressed={!keyValue}
                onClick={() => setDatabase({ keyValue: false })}
              >
                URI
              </Button>
              <Button
                variant={keyValue ? 'filled' : 'default'}
                aria-pressed={keyValue}
                onClick={() => setDatabase({ keyValue: true })}
              >
                key=value
              </Button>
            </div>
            <span className='theme-fivem-entry-hint'>{t(`${base}.database.formatDescription`, {})}</span>
          </div>
        </div>
      </AsideSection>

      <AsideSection title={t(`${base}.keys.title`, {})} description={t(`${base}.keys.description`, {})}>
        <div className='grid gap-6'>
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
            inputWrapperOrder={BELOW}
            placeholder=''
            value={settings.licenseKey}
            onChange={(e) => set('licenseKey', e.currentTarget.value)}
            maxLength={255}
            autoComplete='new-password'
          />
          <PasswordInput
            label={t(`${base}.keys.steam`, {})}
            description={t(`${base}.keys.steamDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder=''
            value={settings.steamKey}
            onChange={(e) => set('steamKey', e.currentTarget.value)}
            maxLength={255}
            autoComplete='new-password'
          />
          <PasswordInput
            label={t(`${base}.keys.tebex`, {})}
            description={t(`${base}.keys.tebexDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder=''
            value={settings.tebexSecret}
            onChange={(e) => set('tebexSecret', e.currentTarget.value)}
            maxLength={255}
            autoComplete='new-password'
          />
        </div>
      </AsideSection>

      <RowList
        list='resources'
        rows={resources}
        setRows={setResources}
        blank={{ key: null, name: '', enabled: true }}
        columns={[t(`${base}.resources.name`, {}), t(`${base}.resources.enabled`, {})]}
        render={(row, update) => (
          <>
            <TextInput
              aria-label={t(`${base}.resources.name`, {})}
              placeholder='chat'
              value={row.name}
              onChange={(e) => update({ ...row, name: e.currentTarget.value })}
              error={row.name.trim() !== '' && !NAME.test(row.name.trim())}
              maxLength={255}
            />
            <Checkbox
              className='theme-fivem-check'
              label={t(`${base}.resources.enabled`, {})}
              checked={row.enabled}
              onChange={(e) => update({ ...row, enabled: e.currentTarget.checked })}
            />
          </>
        )}
      />

      <RowList
        list='principals'
        rows={principals}
        setRows={setPrincipals}
        blank={{ key: null, child: '', parent: '' }}
        columns={[t(`${base}.principals.child`, {}), t(`${base}.principals.parent`, {})]}
        render={(row, update) => (
          <>
            <TextInput
              aria-label={t(`${base}.principals.child`, {})}
              placeholder='identifier.fivem:1'
              value={row.child}
              onChange={(e) => update({ ...row, child: e.currentTarget.value })}
              error={row.child.trim() !== '' && !PRINCIPAL.test(row.child.trim())}
              maxLength={255}
            />
            <TextInput
              aria-label={t(`${base}.principals.parent`, {})}
              placeholder='group.admin'
              value={row.parent}
              onChange={(e) => update({ ...row, parent: e.currentTarget.value })}
              error={row.parent.trim() !== '' && !PRINCIPAL.test(row.parent.trim())}
              maxLength={255}
            />
          </>
        )}
      />

      <RowList
        list='aces'
        rows={aces}
        setRows={setAces}
        blank={{ key: null, principal: '', object: '', access: 'allow' }}
        columns={[t(`${base}.aces.principal`, {}), t(`${base}.aces.object`, {}), t(`${base}.aces.access`, {})]}
        render={(row, update) => (
          <>
            <TextInput
              aria-label={t(`${base}.aces.principal`, {})}
              placeholder='group.admin'
              value={row.principal}
              onChange={(e) => update({ ...row, principal: e.currentTarget.value })}
              error={row.principal.trim() !== '' && !PRINCIPAL.test(row.principal.trim())}
              maxLength={255}
            />
            <TextInput
              aria-label={t(`${base}.aces.object`, {})}
              placeholder='command'
              value={row.object}
              onChange={(e) => update({ ...row, object: e.currentTarget.value })}
              error={row.object.trim() !== '' && !PRINCIPAL.test(row.object.trim())}
              maxLength={255}
            />
            <Select
              aria-label={t(`${base}.aces.access`, {})}
              placeholder=''
              data={[
                { value: 'allow', label: t(`${base}.aces.allow`, {}) },
                { value: 'deny', label: t(`${base}.aces.deny`, {}) },
              ]}
              value={row.access}
              onChange={(value) => update({ ...row, access: value === 'deny' ? 'deny' : 'allow' })}
              allowDeselect={false}
            />
          </>
        )}
      />

      <div>
        <Button onClick={save} loading={saving} disabled={!config.hash}>
          {t(`${base}.button.save`, {})}
        </Button>
      </div>
    </div>
  );
}

/** The whole file in an editor, for what the form does not cover (Pelican's "Edit as text"). */
function RawEditor({
  path,
  opened,
  onClose,
  onSaved,
}: {
  path: string;
  opened: boolean;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const [content, setContent] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!opened) return;

    setContent(null);
    getFileContent(server.uuid, `/${path}`)
      .then((blob) => blob.text())
      .then(setContent)
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
        onClose();
      });
  }, [opened, path]);

  const save = () => {
    if (content === null) return;

    setSaving(true);
    saveFileContent(server.uuid, `/${path}`, content.replace(/\r\n/g, '\n'))
      .then(() => {
        addToast(t(`${base}.toast.saved`, {}), 'success');
        onClose();
        onSaved();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSaving(false));
  };

  return (
    <Modal opened={opened} onClose={onClose} title={`/${path}`} size='80rem'>
      <div className='flex flex-col gap-4'>
        <p className='theme-eco-muted'>{t(`${base}.raw.description`, {})}</p>
        {content === null ? (
          <Spinner.Centered />
        ) : (
          <div className='rounded-md overflow-hidden'>
            <MonacoEditor
              height='60vh'
              theme='vs-dark'
              value={content}
              options={{
                stickyScroll: { enabled: false },
                minimap: { enabled: false },
                codeLens: false,
                scrollBeyondLastLine: false,
                smoothScrolling: false,
                inertialScroll: true,
              }}
              onChange={(value) => setContent(value ?? '')}
              defaultLanguage='ini'
            />
          </div>
        )}
      </div>

      <ModalFooter>
        <Button onClick={save} loading={saving} disabled={content === null}>
          {t(`${base}.button.save`, {})}
        </Button>
        <Button variant='default' onClick={onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

export default function ServerFivemConfig() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const canRestart = useServerCan('control.restart');
  const canEditText = useServerCan(['files.read-content', 'files.update'], false);
  const canFrameworks = useServerCan('fivem.frameworks');
  const [restarting, setRestarting] = useState(false);
  const [editing, setEditing] = useState(false);

  // silent: the page says it when the node does not answer, the rest goes to ResourceView
  const config = useResource({
    queryKey: queryKeys.server(server.uuid).fivem.config(),
    queryFn: () => getFivemConfig(server.uuid),
    silent: true,
  });
  const path = config.data?.path ?? null;
  const unreachable = !config.data && getHttpStatus(config.error) === 502;

  // over the API like the console's power buttons: a start gate holding the start answers with its page
  const restart = () =>
    sendPowerAction(server.uuid, 'restart').then(({ url }) => {
      setRestarting(false);
      if (url?.startsWith('/')) {
        navigate(url);
      } else if (url) {
        window.location.assign(url);
      } else {
        addToast(t(`${base}.toast.restarting`, {}), 'success');
      }
    });

  return (
    <ServerContentContainer
      title={t(`${base}.title`, {})}
      subtitle={path ? t(`${base}.subtitle`, { file: `/${path}` }) : undefined}
      contentRight={
        path && (canRestart || canEditText) ? (
          <>
            {canRestart && (
              <Tooltip label={t(`${base}.button.restart`, {})}>
                <ActionIcon
                  className='theme-fivem-x'
                  variant='subtle'
                  size='input-sm'
                  aria-label={t(`${base}.button.restart`, {})}
                  onClick={() => setRestarting(true)}
                >
                  <FontAwesomeIcon icon={faArrowsRotate} />
                </ActionIcon>
              </Tooltip>
            )}
            {canEditText && (
              <Button variant='default' onClick={() => setEditing(true)}>
                {t(`${base}.button.editText`, {})}
              </Button>
            )}
          </>
        ) : undefined
      }
    >
      <ConfirmationModal
        opened={restarting}
        onClose={() => setRestarting(false)}
        title={t(`${base}.restart.title`, {})}
        confirm={t(`${base}.button.restart`, {})}
        confirmColor='blue'
        onConfirmed={restart}
      >
        {t(`${base}.restart.description`, {})}
      </ConfirmationModal>
      {path && <RawEditor path={path} opened={editing} onClose={() => setEditing(false)} onSaved={config.refetch} />}

      {unreachable ? (
        <TitleCard title={t(`${base}.unreachable.title`, {})}>
          <p className='theme-eco-text'>{t(`${base}.unreachable.description`, {})}</p>
        </TitleCard>
      ) : (
        <ResourceView resource={config}>
          {(data) =>
            data.path === null ? (
              <TitleCard title={t(`${base}.empty.title`, {})}>
                <p className='theme-eco-muted'>{t(`${base}.empty.description`, {})}</p>
                {canFrameworks && (
                  <NavLink to={`/server/${server.uuidShort}/fivem/frameworks`} className='mt-4 inline-block'>
                    <Button>{t(`${base}.empty.frameworks`, {})}</Button>
                  </NavLink>
                )}
              </TitleCard>
            ) : (
              <ConfigForm config={data} onSaved={config.refetch} />
            )
          }
        </ResourceView>
      )}
    </ServerContentContainer>
  );
}
