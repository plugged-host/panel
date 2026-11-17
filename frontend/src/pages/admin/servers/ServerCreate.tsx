import {
  faAddressCard,
  faDownload,
  faEgg,
  faIcons,
  faInfoCircle,
  faNetworkWired,
  faPlay,
  faStopwatch,
  faWrench,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useEffect, useMemo, useRef, useState } from 'react';
import { z } from 'zod';
import getBackupConfigurations from '@/api/admin/backup-configurations/getBackupConfigurations.ts';
import searchEggs, { adminEggWithNestSchema } from '@/api/admin/eggs/searchEggs.ts';
import getEggVariables from '@/api/admin/nests/eggs/variables/getEggVariables.ts';
import getNests from '@/api/admin/nests/getNests.ts';
import getAvailableNodeAllocations from '@/api/admin/nodes/allocations/getAvailableNodeAllocations.ts';
import getNodes from '@/api/admin/nodes/getNodes.ts';
import getPlans from '@/api/admin/plans/getPlans.ts';
import createServer from '@/api/admin/servers/createServer.ts';
import getUsers from '@/api/admin/users/getUsers.ts';
import { getEmptyPaginationSet, httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import { AdvancedModeToggle, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import MultiSelect from '@/elements/input/MultiSelect.tsx';
import Select from '@/elements/input/Select.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import VariableContainer from '@/elements/VariableContainer.tsx';
import { formatAllocation } from '@/lib/domain/server.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminBackupConfigurationSchema } from '@/lib/schemas/admin/backupConfigurations.ts';
import { adminEggVariableSchema } from '@/lib/schemas/admin/eggs.ts';
import { adminNestSchema } from '@/lib/schemas/admin/nests.ts';
import { adminNodeAllocationSchema, adminNodeSchema } from '@/lib/schemas/admin/nodes.ts';
import { AdminServer, adminServerCreateSchema } from '@/lib/schemas/admin/servers.ts';
import { fullUserSchema } from '@/lib/schemas/user.ts';
import EggRepositoryCatalogModal, {
  EggRepositoryCatalogInstallResult,
} from '@/pages/admin/egg-repositories/eggs/modals/EggRepositoryCatalogModal.tsx';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useResourceForm } from '@/plugins/resource/useResourceForm.ts';
import { useSearchableResource } from '@/plugins/resource/useSearchableResource.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { serverCreateEmptyFormValues, useEggDefaults, useServerFormFields } from './serverFormValues.tsx';

type ServerCreateFormValues = z.infer<typeof adminServerCreateSchema>;

export default function ServerCreate() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const canReadNodes = useAdminCan('nodes.read');
  const canReadNodeAllocations = useAdminCan('nodes.allocations');
  const canReadUsers = useAdminCan('users.read');
  const canReadNests = useAdminCan('nests.read');
  const canReadEggs = useAdminCan('eggs.read');
  const canReadBackupConfigurations = useAdminCan('backup-configurations.read');
  const canReadPlans = useAdminCan('plans.read'); // plugged.host

  const [isValid, setIsValid] = useState(false);
  const [openModal, setOpenModal] = useState<'confirm-no-allocation' | null>(null);
  const [confirmStay, setConfirmStay] = useState(false);

  const form = useFormEngine<ServerCreateFormValues>('admin.servers.create', {
    schema: adminServerCreateSchema.unwrap(),
    mode: 'uncontrolled',
    initialValues: serverCreateEmptyFormValues,
    onValuesChange: () => setIsValid(form.isValid()),
  });

  const { loading, doCreateOrUpdate } = useResourceForm<ServerCreateFormValues, AdminServer>({
    form,
    createFn: () => createServer(form.getTransformedValues()),
    doUpdate: false,
    basePath: '/admin/servers',
    resourceName: t('pages.admin.servers.resourceName', {}),
    toResetOnStay: ['allocationUuid', 'allocationUuids'],
  });

  // plugged.host: a dedicated plan fills in its size; the panel picks the server's threads on the
  // node (docs/features/dedicated.md)
  const { data: dedicatedPlans } = useResource({
    queryKey: [...queryKeys.admin.plans.all(), 'dedicated'],
    queryFn: async () => (await getPlans(1, undefined, 100)).data.filter((plan) => plan.dedicated),
    enabled: canReadPlans,
    silent: true,
  });
  const [planUuid, setPlanUuid] = useState<string | null>(null);
  const pickPlan = (uuid: string | null) => {
    setPlanUuid(uuid);
    form.setFieldValue('planUuid', uuid);
    const plan = dedicatedPlans?.find((plan) => plan.uuid === uuid);
    if (plan) {
      form.setFieldValue('limits.cpu', plan.cpu);
      form.setFieldValue('limits.memory', plan.memory);
      form.setFieldValue('limits.memoryOverhead', 0);
      form.setFieldValue('limits.swap', 0);
      form.setFieldValue('limits.disk', plan.disk);
      form.setFieldValue('pinnedCpus', []);
    }
  };

  const doSave = (stay: boolean) => {
    if (!form.getValues().allocationUuid) {
      setConfirmStay(stay);
      setOpenModal('confirm-no-allocation');
    } else {
      doCreateOrUpdate(stay);
    }
  };

  const [eggVariablesLoading, setEggVariablesLoading] = useState(false);
  const [selectedNestUuid, setSelectedNestUuid] = useState<string | null>('');
  const [eggVariables, setEggVariables] = useState<z.infer<typeof adminEggVariableSchema>[]>([]);
  const [catalogOpen, setCatalogOpen] = useState(false);

  const [selectedEggUuid, setSelectedEggUuid] = useState('');
  const [selectedNodeUuid, setSelectedNodeUuid] = useState('');
  const eggsWithNestsRef = useRef<z.infer<typeof adminEggWithNestSchema>[]>([]);
  form.watch('eggUuid', ({ value }) => {
    setSelectedEggUuid(value);

    const item = eggsWithNestsRef.current.find((item) => item.egg.uuid === value);
    if (item) {
      setSelectedNestUuid(item.nest.uuid);
    }
  });
  form.watch('nodeUuid', ({ value }) => setSelectedNodeUuid(value));

  const nodes = useSearchableResource<z.infer<typeof adminNodeSchema>>({
    queryKey: queryKeys.admin.nodes.all(),
    fetcher: (search) => getNodes(1, search),
    canRequest: canReadNodes,
  });
  const users = useSearchableResource<z.infer<typeof fullUserSchema>>({
    queryKey: queryKeys.admin.users.all(),
    fetcher: (search) => getUsers(1, search),
    canRequest: canReadUsers,
  });
  const nests = useSearchableResource<z.infer<typeof adminNestSchema>>({
    queryKey: queryKeys.admin.nests.all(),
    fetcher: (search) => getNests(1, search),
    canRequest: canReadNests,
  });
  const eggsWithNests = useSearchableResource<z.infer<typeof adminEggWithNestSchema>>({
    queryKey: queryKeys.admin.eggs.list(selectedNestUuid),
    fetcher: (search) => searchEggs(selectedNestUuid || null, 1, search),
    canRequest: canReadEggs,
  });
  useEffect(() => {
    eggsWithNestsRef.current = eggsWithNests.items;
  }, [eggsWithNests.items]);
  const eggs = useMemo(
    () => ({
      items: eggsWithNests.items.map((item) => item.egg),
      loading: eggsWithNests.loading,
      search: eggsWithNests.search,
      setSearch: eggsWithNests.setSearch,
    }),
    [eggsWithNests.items, eggsWithNests.loading, eggsWithNests.search, eggsWithNests.setSearch],
  );
  const eggNests = useMemo(
    () => new Map(eggsWithNests.items.map((item) => [item.egg.uuid, item.nest])),
    [eggsWithNests.items],
  );
  const noEggsInstalled =
    canReadEggs &&
    !eggsWithNests.loading &&
    eggsWithNests.items.length === 0 &&
    !eggsWithNests.search &&
    !selectedNestUuid;

  const onEggInstalled = ({ egg, nest }: EggRepositoryCatalogInstallResult) => {
    setSelectedNestUuid(nest.uuid);
    eggsWithNests.setSearch(egg.name);
    form.setFieldValue('eggUuid', egg.uuid);
    form.setFieldValue('image', Object.values(egg.dockerImages)[0] ?? '');
    form.setFieldValue('startup', egg.startupCommands['Default'] || Object.values(egg.startupCommands)[0] || '');
  };
  const availablePrimaryAllocations = useSearchableResource<z.infer<typeof adminNodeAllocationSchema>>({
    queryKey: selectedNodeUuid
      ? queryKeys.admin.nodes.availableAllocations(selectedNodeUuid)
      : ['admin', 'nodes', 'primary-allocations'],
    fetcher: (search) =>
      selectedNodeUuid
        ? getAvailableNodeAllocations(selectedNodeUuid, 1, search)
        : Promise.resolve(getEmptyPaginationSet()),
    deps: [selectedNodeUuid],
    canRequest: canReadNodes && canReadNodeAllocations,
  });
  const availableAllocations = useSearchableResource<z.infer<typeof adminNodeAllocationSchema>>({
    queryKey: selectedNodeUuid
      ? queryKeys.admin.nodes.availableAllocations(selectedNodeUuid)
      : ['admin', 'nodes', 'allocations'],
    fetcher: (search) =>
      selectedNodeUuid
        ? getAvailableNodeAllocations(selectedNodeUuid, 1, search)
        : Promise.resolve(getEmptyPaginationSet()),
    deps: [selectedNodeUuid],
    canRequest: canReadNodes && canReadNodeAllocations,
  });
  const backupConfigurations = useSearchableResource<z.infer<typeof adminBackupConfigurationSchema>>({
    queryKey: queryKeys.admin.backupConfigurations.all(),
    fetcher: (search) => getBackupConfigurations(1, search),
    canRequest: canReadBackupConfigurations,
  });

  const eggImages = eggs.items.find((egg) => egg.uuid === selectedEggUuid)?.dockerImages || {};

  useEggDefaults(form, eggs, selectedEggUuid);

  useEffect(() => {
    if (!selectedNestUuid || !selectedEggUuid) {
      return;
    }

    setEggVariablesLoading(true);
    getEggVariables(selectedNestUuid, selectedEggUuid)
      .then((variables) => {
        setEggVariables(variables);
      })
      .catch((err) => {
        addToast(httpErrorToHuman(err), 'error');
      })
      .finally(() => setEggVariablesLoading(false));
  }, [selectedNestUuid, selectedEggUuid]);

  const { basicInfoFields, serverAssignmentFields, resourceLimitsFields, serverConfigFields, featureLimitsFields } =
    useServerFormFields<ServerCreateFormValues>({
      mode: 'create',
      form,
      nodes,
      users,
      nests,
      eggs,
      backupConfigurations,
      canReadNodes,
      canReadUsers,
      canReadNests,
      canReadEggs,
      canReadBackupConfigurations,
      selectedNestUuid,
      setSelectedNestUuid,
      eggImages,
      nodeUuid: selectedNodeUuid,
      eggNests,
      onEggInstallClick: () => setCatalogOpen(true),
    });

  return (
    <AdminContentContainer
      title={t('pages.admin.servers.tabs.general.page.titleCreate', {})}
      titleOrder={2}
      registry={window.extensionContext.extensionRegistry.pages.admin.servers.create.container}
      contentRight={<AdvancedModeToggle />}
    >
      <ConfirmationModal
        opened={openModal === 'confirm-no-allocation'}
        onClose={() => setOpenModal(null)}
        title={t('pages.admin.servers.tabs.general.page.modal.confirmNoAllocation.title', {})}
        confirm={t('pages.admin.servers.tabs.general.page.modal.confirmNoAllocation.button.confirm', {})}
        onConfirmed={() => {
          setOpenModal(null);
          doCreateOrUpdate(confirmStay);
        }}
      >
        {t('pages.admin.servers.tabs.general.page.modal.confirmNoAllocation.content', {})}
      </ConfirmationModal>

      <EggRepositoryCatalogModal
        opened={catalogOpen}
        onClose={() => setCatalogOpen(false)}
        confirmLabel={t('pages.admin.eggRepositories.tabs.eggs.page.modal.catalog.buttonSelect', {})}
        onInstalled={onEggInstalled}
      />

      <form onSubmit={form.onSubmit(() => doSave(false))}>
        <Stack mt='16'>
          {noEggsInstalled && (
            <EmptyState
              icon={faEgg}
              title={t('pages.admin.servers.tabs.general.page.empty.noEggs.title', {})}
              description={t('pages.admin.servers.tabs.general.page.empty.noEggs.description', {})}
            >
              <AdminCan action={['eggs.create', 'egg-repositories.read']}>
                <Button leftSection={<FontAwesomeIcon icon={faDownload} />} onClick={() => setCatalogOpen(true)}>
                  {t('pages.admin.servers.tabs.general.page.empty.noEggs.button', {})}
                </Button>
              </AdminCan>
            </EmptyState>
          )}

          <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
            <TitleCard
              title={t('pages.admin.servers.tabs.general.page.card.basicInformation', {})}
              icon={<FontAwesomeIcon icon={faInfoCircle} />}
            >
              <FormEngine form={form} fields={basicInfoFields} />
            </TitleCard>

            <TitleCard
              title={t('pages.admin.servers.tabs.general.page.card.serverAssignment', {})}
              icon={<FontAwesomeIcon icon={faAddressCard} />}
            >
              <FormEngine form={form} fields={serverAssignmentFields} />
            </TitleCard>

            <TitleCard
              title={t('pages.admin.servers.tabs.general.page.card.resourceLimits', {})}
              icon={<FontAwesomeIcon icon={faStopwatch} />}
            >
              {/* plugged.host: docs/features/dedicated.md */}
              {!!dedicatedPlans?.length && (
                <Select
                  mb='md'
                  clearable
                  label={t('pages.admin.servers.tabs.general.page.form.dedicatedPlan', {})}
                  description={t('pages.admin.servers.tabs.general.page.form.dedicatedPlanDescription', {})}
                  data={dedicatedPlans.map((plan) => ({ label: plan.name, value: plan.uuid }))}
                  value={planUuid}
                  onChange={pickPlan}
                />
              )}
              <FormEngine form={form} fields={resourceLimitsFields} />
            </TitleCard>

            <TitleCard
              title={t('pages.admin.servers.tabs.general.page.card.serverConfiguration', {})}
              icon={<FontAwesomeIcon icon={faWrench} />}
            >
              <FormEngine form={form} fields={serverConfigFields} />
            </TitleCard>

            <TitleCard
              title={t('pages.admin.servers.tabs.general.page.card.featureLimits', {})}
              icon={<FontAwesomeIcon icon={faIcons} />}
            >
              <FormEngine form={form} fields={featureLimitsFields} />
            </TitleCard>

            <TitleCard
              title={t('pages.admin.servers.tabs.general.page.card.allocations', {})}
              icon={<FontAwesomeIcon icon={faNetworkWired} />}
            >
              <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
                <Select
                  label={t('common.form.primaryAllocation', {})}
                  disabled={!selectedNodeUuid}
                  data={availablePrimaryAllocations.items
                    .filter((alloc) => !form.getValues().allocationUuids.includes(alloc.uuid))
                    .map((alloc) => ({
                      label: formatAllocation(alloc),
                      value: alloc.uuid,
                    }))}
                  searchable
                  searchValue={availablePrimaryAllocations.search}
                  onSearchChange={availablePrimaryAllocations.setSearch}
                  allowDeselect
                  key={form.key('allocationUuid')}
                  {...form.getInputProps('allocationUuid')}
                />
                <MultiSelect
                  label={t('common.form.additionalAllocations', {})}
                  disabled={!selectedNodeUuid}
                  data={availableAllocations.items
                    .filter((alloc) => alloc.uuid !== form.getValues().allocationUuid)
                    .map((alloc) => ({
                      label: formatAllocation(alloc),
                      value: alloc.uuid,
                    }))}
                  searchable
                  searchValue={availableAllocations.search}
                  onSearchChange={availableAllocations.setSearch}
                  key={form.key('allocationUuids')}
                  {...form.getInputProps('allocationUuids')}
                />
              </div>
            </TitleCard>

            <TitleCard
              title={t('pages.admin.servers.tabs.general.page.card.variables', {})}
              icon={<FontAwesomeIcon icon={faPlay} />}
              className='col-span-full'
            >
              <Stack>
                {!selectedNestUuid || !selectedEggUuid ? (
                  <Alert>{t('pages.admin.servers.tabs.general.page.alert.selectEggForVariables', {})}</Alert>
                ) : eggVariablesLoading ? (
                  <Spinner.Centered />
                ) : (
                  <div className='grid grid-cols-1 xl:grid-cols-2 gap-4'>
                    {eggVariables.map((variable) => (
                      <VariableContainer
                        key={variable.envVariable}
                        variable={{
                          ...variable,
                          value: '',
                          isEditable: variable.userEditable,
                        }}
                        loading={loading}
                        overrideReadonly
                        value={
                          form.getValues().variables.find((v) => v.envVariable === variable.envVariable)?.value ??
                          variable.defaultValue ??
                          ''
                        }
                        setValue={(value) =>
                          form.setFieldValue('variables', (prev) => [
                            ...prev.filter((v) => v.envVariable !== variable.envVariable),
                            { envVariable: variable.envVariable, value },
                          ])
                        }
                      />
                    ))}
                  </div>
                )}
              </Stack>
            </TitleCard>
          </div>

          <Group>
            <AdminCan action='servers.create' cantSave>
              <Button type='submit' disabled={!isValid} loading={loading}>
                {t('common.button.save', {})}
              </Button>
              <Button onClick={() => doSave(true)} disabled={!isValid} loading={loading}>
                {t('common.button.saveAndStay', {})}
              </Button>
            </AdminCan>
          </Group>
        </Stack>
      </form>
    </AdminContentContainer>
  );
}
