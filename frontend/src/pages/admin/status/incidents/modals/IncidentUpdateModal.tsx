// plugged.host: Admin › Status › Incidents: posting an update (a new status and what happened).
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import postStatusIncidentUpdate from '@/api/admin/status/postStatusIncidentUpdate.ts';
import Button from '@/elements/buttons/Button.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminStatusIncident, adminStatusIncidentPostUpdateSchema } from '@/lib/schemas/admin/status.ts';
import { useModalForm } from '@/plugins/form/useModalForm.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { incidentStatusOptions } from '../../statusLabels.ts';

type UpdateFormValues = z.infer<typeof adminStatusIncidentPostUpdateSchema>;

export default function IncidentUpdateModal({
  incident,
  opened,
  onClose,
  ...props
}: ModalProps & { incident: AdminStatusIncident }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();

  const initialValues: UpdateFormValues = { status: incident.status, message: '' };

  const { form, handleClose, handleSubmit, loading, isDirty } = useModalForm<UpdateFormValues>({
    schema: adminStatusIncidentPostUpdateSchema,
    initialValues,
    opened,
    onClose,
    hydrate: () => initialValues,
    onSubmit: async (values) => {
      await postStatusIncidentUpdate(incident.uuid, adminStatusIncidentPostUpdateSchema.parse(values));
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.status.incidents() });
      addToast(t('pages.admin.status.tabs.incidents.page.toast.updated', {}), 'success');
    },
  });

  const fields: FieldDef<UpdateFormValues>[] = [
    {
      type: 'select',
      name: 'status',
      label: t('common.table.columns.status', {}),
      description: t('pages.admin.status.tabs.incidents.page.form.statusDescription', {}),
      options: incidentStatusOptions(),
      required: true,
      colSpan: 'full',
      props: { allowDeselect: false },
    },
    {
      type: 'textarea',
      name: 'message',
      label: t('pages.admin.status.tabs.incidents.page.form.message', {}),
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
      title={t('pages.admin.status.tabs.incidents.page.modal.update.title', {})}
    >
      <Stack gap='md'>
        <Text size='sm' fw={500}>
          {incident.title}
        </Text>

        {incident.updates.length > 0 && (
          <Stack gap='xs'>
            {incident.updates.slice(0, 3).map((update) => (
              <Text key={update.uuid} size='xs' c='dimmed'>
                {update.message}
              </Text>
            ))}
          </Stack>
        )}

        <FormEngine form={form} fields={fields} />

        <ModalFooter>
          <Button type='submit' disabled={!form.isValid()} loading={loading}>
            {t('pages.admin.status.tabs.incidents.page.button.postUpdate', {})}
          </Button>
          <Button variant='default' onClick={handleClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </FormModal>
  );
}
