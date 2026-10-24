// plugged.host: one-click frameworks, txAdmin's recipes installed into a new folder, laid out like the Pelican panel's
// Framework installer (docs/features/fivem.md#frameworks)
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { NavLink } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import dismissFivemFramework from '@/api/server/fivem/dismissFivemFramework.ts';
import getFivemFrameworks from '@/api/server/fivem/getFivemFrameworks.ts';
import installFivemFramework from '@/api/server/fivem/installFivemFramework.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import PasswordInput from '@/elements/input/PasswordInput.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import ConditionalTooltip from '@/elements/overlays/ConditionalTooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  FivemDatabaseChoice,
  fivemFrameworkInstallSchema,
  fivemFrameworksSchema,
  fivemRecipeSchema,
} from '@/lib/schemas/server/fivem.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Frameworks = z.infer<typeof fivemFrameworksSchema>;
type Recipe = z.infer<typeof fivemRecipeSchema>;
type Install = z.infer<typeof fivemFrameworkInstallSchema>;

const base = 'pages.server.fivem.frameworks';
const LICENSE = /^[A-Za-z0-9_-]+$/;
/** Filament's helper text sits under the field, not between label and field. */
const BELOW: ('label' | 'input' | 'description' | 'error')[] = ['label', 'input', 'description', 'error'];

function running(install: Install | null | undefined) {
  return !!install && install.step !== 'done' && install.step !== 'failed';
}

/** The step count the Pelican page shows: 0 while preparing, the task running, then all of them. */
function stepOf(install: Install) {
  const total = Math.max(install.total, 1);
  const step = install.step === 'prepare' ? 0 : install.step === 'tasks' ? install.task : total;

  return { step: Math.min(step, total), total };
}

function InstallModal({
  recipe,
  frameworks,
  onClose,
}: {
  recipe: Recipe | null;
  frameworks: Frameworks;
  onClose: () => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const [serverName, setServerName] = useState(server.name);
  const [license, setLicense] = useState(frameworks.license ?? '');
  const [database, setDatabase] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const databases = [
    ...(frameworks.canCreateDatabase ? [{ value: 'new', label: t(`${base}.install.databaseNew`, {}) }] : []),
    ...frameworks.databases.map((database) => ({
      value: database.uuid,
      label: t(`${base}.install.databaseExisting`, { name: database.name }),
    })),
  ];

  useEffect(() => {
    if (!recipe) return;
    setServerName(server.name);
    setLicense(frameworks.license ?? '');
    setDatabase(databases[0]?.value ?? null);
  }, [recipe]);

  const choice: FivemDatabaseChoice | null =
    !recipe?.needsDatabase || !database
      ? null
      : database === 'new'
        ? { mode: 'new' }
        : { mode: 'existing', uuid: database };
  const valid =
    serverName.trim().length > 0 && LICENSE.test(license.trim()) && (!recipe?.needsDatabase || choice !== null);

  const install = () => {
    if (!recipe) return;
    setLoading(true);

    installFivemFramework(server.uuid, {
      recipe: recipe.id,
      serverName: serverName.trim(),
      license: license.trim(),
      database: choice,
    })
      .then(() => {
        onClose();
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).fivem.frameworks() });
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <Modal
      opened={recipe !== null}
      onClose={onClose}
      title={t(`${base}.install.title`, { recipe: recipe?.name ?? '' })}
    >
      <div className='flex flex-col gap-6'>
        <p className='theme-eco-muted'>
          {frameworks.current
            ? t(`${base}.install.description`, { folder: frameworks.current })
            : t(`${base}.install.descriptionFresh`, {})}
        </p>
        <TextInput
          label={t(`${base}.install.serverName`, {})}
          description={t(`${base}.install.serverNameDescription`, {})}
          inputWrapperOrder={BELOW}
          placeholder=''
          value={serverName}
          onChange={(e) => setServerName(e.currentTarget.value)}
          maxLength={64}
          required
        />
        <PasswordInput
          label={t(`${base}.install.license`, {})}
          description={t(`${base}.install.licenseDescription`, {})}
          inputWrapperOrder={BELOW}
          placeholder=''
          value={license}
          onChange={(e) => setLicense(e.currentTarget.value)}
          error={license.trim() !== '' && !LICENSE.test(license.trim())}
          maxLength={128}
          autoComplete='new-password'
          required
        />
        {recipe?.needsDatabase && (
          <Select
            label={t(`${base}.install.database`, {})}
            description={t(`${base}.install.databaseDescription`, {})}
            inputWrapperOrder={BELOW}
            placeholder=''
            data={databases}
            value={database}
            onChange={setDatabase}
            allowDeselect={false}
            required
          />
        )}
      </div>

      <ModalFooter>
        <Button onClick={install} loading={loading} disabled={!valid}>
          {t(`${base}.install.button`, {})}
        </Button>
        <Button variant='default' onClick={onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

/** The install in place of the recipes, on one card: its steps while it runs, then how it ended. */
function Progressing({ install }: { install: Install }) {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const [dismissing, setDismissing] = useState(false);

  const dismiss = () => {
    setDismissing(true);
    dismissFivemFramework(server.uuid)
      .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).fivem.frameworks() }))
      .finally(() => setDismissing(false));
  };

  const isRunning = running(install);
  const { step, total } = stepOf(install);

  return (
    <div className='theme-fivem-run'>
      <div className='theme-fivem-run-head'>
        <span className='theme-fivem-run-title'>{t(`${base}.progress.installing`, { recipe: install.recipe })}</span>
        {isRunning && <span className='theme-fivem-run-count'>{t(`${base}.progress.stepOf`, { step, total })}</span>}
      </div>

      {isRunning ? (
        <>
          <div className='theme-fivem-progress'>
            <i style={{ width: `${Math.round((step / total) * 100)}%` }} />
          </div>
          <div className='theme-fivem-step'>
            {install.step === 'prepare' || install.step === 'config' || install.step === 'restart'
              ? t(`${base}.progress.${install.step}`, {})
              : (install.current ?? '')}
          </div>
          <p className='theme-eco-muted'>{t(`${base}.progress.leave`, {})}</p>
        </>
      ) : install.step === 'done' ? (
        <>
          <p className='theme-fivem-ok'>{t(`${base}.progress.done`, { folder: install.deploy ?? '' })}</p>
          {install.needsAdmin && <p className='theme-eco-muted'>{t(`${base}.progress.needsAdmin`, {})}</p>}
          {install.previous && (
            <p className='theme-eco-muted'>{t(`${base}.progress.previousKept`, { folder: install.previous })}</p>
          )}
        </>
      ) : (
        <>
          <p className='theme-fivem-err'>{install.error ?? t(`${base}.progress.failed`, {})}</p>
          <p className='theme-eco-muted'>{t(`${base}.progress.failedNote`, {})}</p>
        </>
      )}

      {install.database && (
        <div className='theme-fivem-log'>{t(`${base}.progress.database`, { name: install.database })}</div>
      )}

      {!isRunning && (
        <div className='flex flex-wrap gap-2'>
          {install.step === 'done' && (
            <NavLink to={`/server/${server.uuidShort}`}>
              <Button variant='default'>{t(`${base}.progress.console`, {})}</Button>
            </NavLink>
          )}
          <Button variant='default' onClick={dismiss} loading={dismissing}>
            {t(`${base}.progress.back`, {})}
          </Button>
        </div>
      )}
    </div>
  );
}

function Recipes({ frameworks }: { frameworks: Frameworks }) {
  const { t, tReact } = useTranslations();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const [installing, setInstalling] = useState<Recipe | null>(null);

  const blocked = (recipe: Recipe) =>
    recipe.needsDatabase && !frameworks.canCreateDatabase && frameworks.databases.length === 0
      ? t(`${base}.noDatabase`, {})
      : null;

  return (
    <>
      <InstallModal recipe={installing} frameworks={frameworks} onClose={() => setInstalling(null)} />

      <p className='theme-fivem-current'>
        {frameworks.current
          ? tReact(`${base}.current`, { folder: <code className='theme-fivem-code'>{frameworks.current}</code> })
          : t(`${base}.currentNone`, {})}
      </p>

      {frameworks.recipes.length === 0 ? (
        <div className='theme-fivem-empty' data-shape='card'>
          {t(`${base}.indexUnreachable`, {})}
        </div>
      ) : (
        frameworks.recipes.map((recipe) => {
          const reason = blocked(recipe);

          return (
            <div key={recipe.id} className='theme-fivem-recipe'>
              {recipe.logo ? (
                <img
                  className='theme-fivem-logo'
                  src={recipe.logo}
                  alt=''
                  loading='lazy'
                  referrerPolicy='no-referrer'
                />
              ) : (
                <span className='theme-fivem-logo' />
              )}
              <div className='min-w-0'>
                <div className='theme-fivem-recipe-name'>{recipe.name}</div>
                {(recipe.version || recipe.author) && (
                  <div className='theme-fivem-meta'>
                    {recipe.version && <code>v{recipe.version.replace(/^v/, '')}</code>}
                    {recipe.version && recipe.author && ' · '}
                    {recipe.author && t(`${base}.by`, { author: recipe.author })}
                  </div>
                )}
                {recipe.description && <div className='theme-fivem-desc'>{recipe.description}</div>}
                {recipe.needsDatabase && <div className='theme-fivem-meta'>{t(`${base}.needsDatabase`, {})}</div>}
              </div>
              <div>
                <ConditionalTooltip enabled={reason !== null} label={reason}>
                  <Button variant='default' disabled={reason !== null} onClick={() => setInstalling(recipe)}>
                    {t(`${base}.install.button`, {})}
                  </Button>
                </ConditionalTooltip>
              </div>
            </div>
          );
        })
      )}

      <p className='theme-eco-muted'>{t(`${base}.sourceNote`, { app: appName })}</p>
    </>
  );
}

export default function ServerFivemFrameworks() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);

  const query = useQuery({
    queryKey: queryKeys.server(server.uuid).fivem.frameworks(),
    queryFn: () => getFivemFrameworks(server.uuid),
    refetchInterval: (current) => (running(current.state.data?.install) ? 3000 : false),
  });

  // the outcome as a toast too: an install takes minutes and the page may be in the background
  const step = query.data?.install?.step;
  const previousStep = useRef(step);
  useEffect(() => {
    const install = query.data?.install;
    if (install && previousStep.current && running({ ...install, step: previousStep.current })) {
      if (step === 'done') {
        addToast(
          t(`${base}.toast.done`, { recipe: install.recipe, server: server.name, folder: install.deploy ?? '' }),
          'success',
        );
      } else if (step === 'failed') {
        addToast(t(`${base}.toast.failed`, { recipe: install.recipe, server: server.name }), 'error');
      }
    }
    previousStep.current = step;
  }, [step]);

  return (
    <ServerContentContainer title={t(`${base}.title`, {})} subtitle={t(`${base}.subtitle`, {})}>
      <ResourceView
        resource={{ data: query.data, loading: query.isLoading, error: query.error, refetch: () => query.refetch() }}
      >
        {(frameworks) => (
          <div className='flex flex-col gap-4'>
            {/* a running or finished install first: it is never lost behind a node that blinked */}
            {frameworks.install ? (
              <Progressing install={frameworks.install} />
            ) : frameworks.unreachable ? (
              <div className='theme-fivem-empty' data-shape='card'>
                {t(`${base}.unreachable`, {})}
              </div>
            ) : (
              <Recipes frameworks={frameworks} />
            )}
          </div>
        )}
      </ResourceView>
    </ServerContentContainer>
  );
}
