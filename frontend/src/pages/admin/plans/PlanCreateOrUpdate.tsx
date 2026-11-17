// plugged.host: Admin › Plans, one plan's form
import { useState } from 'react';
import { z } from 'zod';
import getLocation from '@/api/admin/locations/getLocation.ts';
import getLocations from '@/api/admin/locations/getLocations.ts';
import getNode from '@/api/admin/nodes/getNode.ts';
import getNodes from '@/api/admin/nodes/getNodes.ts';
import createPlan from '@/api/admin/plans/createPlan.ts';
import deletePlan from '@/api/admin/plans/deletePlan.ts';
import updatePlan from '@/api/admin/plans/updatePlan.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import NumberInput from '@/elements/input/NumberInput.tsx';
import SearchableUuidMultiSelect from '@/elements/input/SearchableUuidMultiSelect.tsx';
import Group from '@/elements/layout/Group.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminPlanSchema, adminPlanUpdateSchema } from '@/lib/schemas/admin/plans.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useResourceForm } from '@/plugins/resource/useResourceForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type PlanFormValues = z.infer<typeof adminPlanUpdateSchema>;

const planEmptyFormValues: PlanFormValues = {
  name: '',
  description: null,
  cpu: 100,
  memory: 2048,
  disk: 10240,
  sort: 0,
  public: true,
  dedicated: false,
  price: null,
  locationUuids: [],
  nodeUuids: [],
};

const planToFormValues = (plan: z.infer<typeof adminPlanSchema>): Partial<PlanFormValues> => ({
  name: plan.name,
  description: plan.description,
  cpu: plan.cpu,
  memory: plan.memory,
  disk: plan.disk,
  sort: plan.sort,
  public: plan.public,
  dedicated: plan.dedicated,
  price: plan.price,
  locationUuids: plan.locationUuids,
  nodeUuids: plan.nodeUuids,
});

export default ({ contextPlan }: { contextPlan?: z.infer<typeof adminPlanSchema> }) => {
  const { t } = useTranslations();
  const [openModal, setOpenModal] = useState<'delete' | null>(null);

  const form = useFormEngine<PlanFormValues>('admin.plans.createOrUpdate', {
    schema: adminPlanUpdateSchema,
    initialValues: planEmptyFormValues,
  });

  const { loading, doCreateOrUpdate, doDelete } = useResourceForm<PlanFormValues, z.infer<typeof adminPlanSchema>>({
    form,
    createFn: () => createPlan(form.getTransformedValues()),
    updateFn: contextPlan ? () => updatePlan(contextPlan.uuid, form.getTransformedValues()) : undefined,
    deleteFn: contextPlan ? () => deletePlan(contextPlan.uuid) : undefined,
    doUpdate: !!contextPlan,
    basePath: '/admin/plans',
    resourceName: t('pages.admin.plans.resourceName', {}),
  });

  useHydrateForm(form, contextPlan, planToFormValues);

  const fields: FieldDef<PlanFormValues>[] = [
    { type: 'text', name: 'name', label: t('common.form.name', {}), required: true },
    { type: 'textarea', name: 'description', label: t('common.form.description', {}), rows: 3 },
    {
      type: 'number',
      name: 'cpu',
      label: t('pages.admin.plans.form.cpu', {}),
      description: form.getValues().dedicated
        ? t('pages.admin.plans.form.cpuDedicatedDescription', {})
        : t('pages.admin.plans.form.cpuDescription', {}),
      required: true,
      props: { min: 1, step: form.getValues().dedicated ? 100 : 1 },
    },
    { type: 'size', name: 'memory', label: t('common.form.memory', {}), required: true, mode: 'mb', min: 1 },
    { type: 'size', name: 'disk', label: t('common.form.disk', {}), required: true, mode: 'mb', min: 1 },
    {
      type: 'number',
      name: 'sort',
      label: t('pages.admin.plans.form.sort', {}),
      description: t('pages.admin.plans.form.sortDescription', {}),
      required: true,
    },
    {
      type: 'switch',
      name: 'public',
      label: t('pages.admin.plans.form.public', {}),
      description: form.getValues().dedicated
        ? t('pages.admin.plans.form.publicDedicatedDescription', {})
        : t('pages.admin.plans.form.publicDescription', {}),
    },
    // plugged.host: paid plans with CPU threads and memory of their own (docs/features/dedicated.md)
    { type: 'divider', name: 'dedicatedDivider', label: t('pages.admin.plans.form.dedicatedSection', {}) },
    {
      type: 'switch',
      name: 'dedicated',
      label: t('pages.admin.plans.form.dedicated', {}),
      description: t('pages.admin.plans.form.dedicatedDescription', {}),
    },
    {
      type: 'custom',
      name: 'price',
      when: (values) => values.dedicated,
      render: (f) => (
        <NumberInput
          label={t('pages.admin.plans.form.price', {})}
          description={t('pages.admin.plans.form.priceDescription', {})}
          min={0}
          decimalScale={2}
          fixedDecimalScale
          value={f.getValues().price === null ? '' : f.getValues().price! / 100}
          onChange={(value) =>
            f.setFieldValue('price', value === '' || value === null ? null : Math.round(Number(value) * 100))
          }
        />
      ),
    },
    {
      type: 'custom',
      name: 'locationUuids',
      colSpan: 'full',
      when: (values) => values.dedicated,
      render: (f) => (
        <SearchableUuidMultiSelect
          label={t('pages.admin.plans.form.locations', {})}
          description={t('pages.admin.plans.form.locationsDescription', {})}
          value={f.getValues().locationUuids}
          onChange={(locationUuids) => f.setFieldValue('locationUuids', locationUuids)}
          listQueryKey={queryKeys.admin.locations.all()}
          detailQueryKey={queryKeys.admin.locations.detail}
          fetchPage={(search) => getLocations(1, search)}
          fetchOne={getLocation}
        />
      ),
    },
    {
      type: 'custom',
      name: 'nodeUuids',
      colSpan: 'full',
      when: (values) => values.dedicated,
      render: (f) => (
        <SearchableUuidMultiSelect
          label={t('pages.admin.plans.form.nodes', {})}
          description={t('pages.admin.plans.form.nodesDescription', {})}
          value={f.getValues().nodeUuids}
          onChange={(nodeUuids) => f.setFieldValue('nodeUuids', nodeUuids)}
          listQueryKey={queryKeys.admin.nodes.all()}
          detailQueryKey={queryKeys.admin.nodes.detail}
          fetchPage={(search) => getNodes(1, search)}
          fetchOne={getNode}
        />
      ),
    },
  ];

  return (
    <AdminContentContainer
      title={
        contextPlan
          ? t('pages.admin.plans.titleUpdate', { name: contextPlan.name })
          : t('pages.admin.plans.titleCreate', {})
      }
      titleOrder={2}
    >
      <ConfirmationModal
        opened={openModal === 'delete'}
        onClose={() => setOpenModal(null)}
        title={t('pages.admin.plans.modal.delete.title', {})}
        confirm={t('common.button.delete', {})}
        onConfirmed={doDelete}
      >
        {t('common.modal.delete.content', { name: form.getValues().name }).md()}
      </ConfirmationModal>

      <form onSubmit={form.onSubmit(() => doCreateOrUpdate(false, queryKeys.admin.plans.all()))}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <AdminCan action={contextPlan ? 'plans.update' : 'plans.create'} cantSave>
            <Button type='submit' disabled={!form.isValid()} loading={loading}>
              {t('common.button.save', {})}
            </Button>
          </AdminCan>
          {contextPlan && (
            <AdminCan action='plans.delete' cantDelete>
              <Button variant='default' onClick={() => setOpenModal('delete')} loading={loading}>
                {t('common.button.delete', {})}
              </Button>
            </AdminCan>
          )}
        </Group>
      </form>
    </AdminContentContainer>
  );
};
