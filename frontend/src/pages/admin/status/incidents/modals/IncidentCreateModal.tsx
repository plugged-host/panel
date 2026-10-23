// plugged.host: Admin › Status › Incidents: posting a new incident with its first update.
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import createStatusIncident from '@/api/admin/status/createStatusIncident.ts';
import Button from '@/elements/buttons/Button.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminStatusComponent, adminStatusIncidentCreateSchema } from '@/lib/schemas/admin/status.ts';
import { useModalForm } from '@/plugins/form/useModalForm.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { incidentImpactOptions, incidentStatusOptions } from '../../statusLabels.ts';

type IncidentFormValues = z.infer<typeof adminStatusIncidentCreateSchema>;

const emptyValues: IncidentFormValues = {
  title: '',
  impact: 'minor',
  status: 'investigating',
  components: [],
  message: '',
};

export default function IncidentCreateModal({
  components,
  opened,
  onClose,
  ...props
}: ModalProps & { components: AdminStatusComponent[] }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();

  const { form, handleClose, handleSubmit, loading, isDirty } = useModalForm<IncidentFormValues>({
    schema: adminStatusIncidentCreateSchema,
    initialValues: emptyValues,
    opened,
    onClose,
    onSubmit: async (values) => {
      await createStatusIncident(adminStatusIncidentCreateSchema.parse(values));
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.status.incidents() });
      addToast(t('pages.admin.status.tabs.incidents.page.toast.created', {}), 'success');
    },
  });

  const fields: FieldDef<IncidentFormValues>[] = [
    {
      type: 'text',
      name: 'title',
      label: t('common.table.columns.title', {}),
      required: true,
      colSpan: 'full',
    },
    {
      type: 'select',
      name: 'impact',
      label: t('pages.admin.status.tabs.incidents.page.column.impact', {}),
      options: incidentImpactOptions(),
      required: true,
      props: { allowDeselect: false },
    },
    {
      type: 'select',
      name: 'status',
      label: t('common.table.columns.status', {}),
      options: incidentStatusOptions(),
      required: true,
      props: { allowDeselect: false },
    },
    {
      type: 'multiselect',
      name: 'components',
      label: t('pages.admin.status.tabs.incidents.page.column.components', {}),
      description: t('pages.admin.status.tabs.incidents.page.form.componentsDescription', {}),
      options: components.map((component) => ({ value: component.uuid, label: component.label })),
      colSpan: 'full',
      props: { searchable: true },
    },
    {
      type: 'textarea',
      name: 'message',
      label: t('pages.admin.status.tabs.incidents.page.form.message', {}),
      description: t('pages.admin.status.tabs.incidents.page.form.messageDescription', {}),
      required: true,
      rows: 4,
      colSpan: 'full',
    },
  ];

  return (
    <FormModal
      {...props}
      opened={opened}
      onClose={handleClose}
      onSubmit={handleSubmit}
      isDirty={isDirty}
      loading={loading}
      title={t('pages.admin.status.tabs.incidents.page.modal.create.title', {})}
    >
      <Stack gap='md'>
        <FormEngine form={form} fields={fields} />

        <ModalFooter>
          <Button type='submit' disabled={!form.isValid()} loading={loading}>
            {t('common.button.create', {})}
          </Button>
          <Button variant='default' onClick={handleClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </FormModal>
  );
}
