// plugged.host: one-click frameworks, txAdmin's recipes installed into a new folder (docs/features/fivem.md#frameworks)
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
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Progress from '@/elements/feedback/Progress.tsx';
import PasswordInput from '@/elements/input/PasswordInput.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  FivemDatabaseChoice,
  fivemFrameworkInstallSchema,
  fivemFrameworksSchema,
  fivemRecipeSchema,
} from '@/lib/schemas/server/fivem.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Frameworks = z.infer<typeof fivemFrameworksSchema>;
type Recipe = z.infer<typeof fivemRecipeSchema>;
type Install = z.infer<typeof fivemFrameworkInstallSchema>;

const base = 'pages.server.fivem.frameworks';
const LICENSE = /^[A-Za-z0-9_-]+$/;

function running(install: Install | null | undefined) {
  return !!install && install.step !== 'done' && install.step !== 'failed';
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
      <Stack gap='md'>
        <Text size='sm' c='dimmed'>
          {frameworks.current
            ? t(`${base}.install.description`, { folder: frameworks.current })
            : t(`${base}.install.descriptionFresh`, {})}
        </Text>
        <TextInput
          label={t(`${base}.install.serverName`, {})}
          description={t(`${base}.install.serverNameDescription`, {})}
          value={serverName}
          onChange={(e) => setServerName(e.currentTarget.value)}
          maxLength={64}
          required
        />
        <PasswordInput
          label={t(`${base}.install.license`, {})}
          description={t(`${base}.install.licenseDescription`, {})}
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
            data={databases}
            value={database}
            onChange={setDatabase}
            allowDeselect={false}
            required
          />
        )}
      </Stack>

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

  if (install.step === 'failed') {
    return (
      <Stack gap='sm'>
        <Text fw={500}>{t(`${base}.progress.failed`, { recipe: install.recipe })}</Text>
        <Text size='sm' c='red'>
          {install.error ?? ''}
        </Text>
        <Text size='sm' c='dimmed'>
          {t(`${base}.progress.failedNote`, {})}
        </Text>
        <Group>
          <Button variant='default' onClick={dismiss} loading={dismissing}>
            {t(`${base}.progress.back`, {})}
          </Button>
        </Group>
      </Stack>
    );
  }

  if (install.step === 'done') {
    return (
      <Stack gap='sm'>
        <Text fw={500}>{t(`${base}.progress.done`, { recipe: install.recipe, folder: install.deploy ?? '' })}</Text>
        {install.needsAdmin && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.progress.needsAdmin`, {})}
          </Text>
        )}
        {install.previous && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.progress.previousKept`, { folder: install.previous })}
          </Text>
        )}
        <Group>
          <NavLink to={`/server/${server.uuidShort}`}>
            <Button>{t(`${base}.progress.console`, {})}</Button>
          </NavLink>
          <Button variant='default' onClick={dismiss} loading={dismissing}>
            {t(`${base}.progress.back`, {})}
          </Button>
        </Group>
      </Stack>
    );
  }

  const stepText =
    install.step === 'tasks'
      ? `${t(`${base}.progress.stepOf`, { step: install.task, total: install.total })}${install.current ? `: ${install.current}` : ''}`
      : t(`${base}.progress.${install.step}`, {});

  return (
    <Stack gap='sm'>
      <Text fw={500}>{t(`${base}.progress.installing`, { recipe: install.recipe })}</Text>
      <Text size='sm'>{stepText}</Text>
      <Progress
        value={install.step === 'tasks' && install.total > 0 ? ((install.task - 1) / install.total) * 100 : NaN}
        indeterminate={install.step !== 'tasks' || install.total === 0}
      />
      {install.database && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.progress.database`, { name: install.database })}
        </Text>
      )}
      <Text size='sm' c='dimmed'>
        {t(`${base}.progress.leave`, {})}
      </Text>
    </Stack>
  );
}

function Recipes({ frameworks }: { frameworks: Frameworks }) {
  const { t } = useTranslations();
  const [installing, setInstalling] = useState<Recipe | null>(null);

  const blocked = (recipe: Recipe) =>
    recipe.needsDatabase && !frameworks.canCreateDatabase && frameworks.databases.length === 0
      ? t(`${base}.noDatabase`, {})
      : null;

  return (
    <Stack gap='md'>
      <InstallModal recipe={installing} frameworks={frameworks} onClose={() => setInstalling(null)} />

      <Text size='sm' c='dimmed'>
        {frameworks.unreachable
          ? t(`${base}.unreachable`, {})
          : frameworks.current
            ? t(`${base}.current`, { folder: frameworks.current })
            : t(`${base}.currentNone`, {})}
      </Text>

      <Table
        columns={[t(`${base}.table.framework`, {}), '']}
        pagination={{
          total: frameworks.recipes.length,
          perPage: Math.max(frameworks.recipes.length, 1),
          page: 1,
          data: frameworks.recipes,
        }}
        empty={
          <Text size='sm' c='dimmed'>
            {t(`${base}.indexUnreachable`, {})}
          </Text>
        }
      >
        {frameworks.recipes.map((recipe) => (
          <TableRow key={recipe.id}>
            <TableData>
              <Group gap='sm' wrap='nowrap' align='flex-start'>
                {recipe.logo && (
                  <img src={recipe.logo} alt='' width={40} height={40} className='rounded-md shrink-0' loading='lazy' />
                )}
                <Stack gap={2}>
                  <Text size='sm' fw={500}>
                    {recipe.name}
                    {recipe.version ? ` ${recipe.version}` : ''}
                  </Text>
                  {recipe.author && (
                    <Text size='xs' c='dimmed'>
                      {t(`${base}.by`, { author: recipe.author })}
                    </Text>
                  )}
                  {recipe.description && <Text size='sm'>{recipe.description}</Text>}
                  {recipe.needsDatabase && (
                    <Text size='xs' c='dimmed'>
                      {blocked(recipe) ?? t(`${base}.needsDatabase`, {})}
                    </Text>
                  )}
                </Stack>
              </Group>
            </TableData>
            <TableData className='text-end'>
              <Button
                variant='default'
                size='xs'
                disabled={blocked(recipe) !== null}
                onClick={() => setInstalling(recipe)}
              >
                {t(`${base}.install.button`, {})}
              </Button>
            </TableData>
          </TableRow>
        ))}
      </Table>

      <Text size='xs' c='dimmed'>
        {t(`${base}.sourceNote`, {})}
      </Text>
    </Stack>
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
        addToast(t(`${base}.progress.done`, { recipe: install.recipe, folder: install.deploy ?? '' }), 'success');
      } else if (step === 'failed') {
        addToast(t(`${base}.progress.failed`, { recipe: install.recipe }), 'error');
      }
    }
    previousStep.current = step;
  }, [step]);

  return (
    <ServerContentContainer title={t(`${base}.title`, {})}>
      <ResourceView
        resource={{ data: query.data, loading: query.isLoading, error: query.error, refetch: () => query.refetch() }}
      >
        {(frameworks) =>
          frameworks.install ? <Progressing install={frameworks.install} /> : <Recipes frameworks={frameworks} />
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
