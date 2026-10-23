// plugged.host: Admin › Status › Overrides: a new override.
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import createStatusOverride from '@/api/admin/status/createStatusOverride.ts';
import Button from '@/elements/buttons/Button.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminStatusComponent, statusComponentStatus } from '@/lib/schemas/admin/status.ts';
import { useModalForm } from '@/plugins/form/useModalForm.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { componentStatusOptions } from '../../statusLabels.ts';

// The select can't hold null: "all" stands for every component.
const ALL = 'all';

const overrideFormSchema = z.object({
  component: z.string().min(1),
  status: statusComponentStatus,
  note: z.string().max(1000),
  starts: z.coerce.date().nullable(),
  ends: z.coerce.date().nullable(),
});

type OverrideFormValues = z.infer<typeof overrideFormSchema>;

const emptyValues: OverrideFormValues = {
  component: ALL,
  status: 'maintenance',
  note: '',
  starts: null,
  ends: null,
};

export default function OverrideCreateModal({
  components,
  opened,
  onClose,
  ...props
}: ModalProps & { components: AdminStatusComponent[] }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();

  const { form, handleClose, handleSubmit, loading, isDirty } = useModalForm<OverrideFormValues>({
    schema: overrideFormSchema,
    initialValues: emptyValues,
    opened,
    onClose,
    onSubmit: async (values) => {
      const parsed = overrideFormSchema.parse(values);
      await createStatusOverride({
        componentUuid: parsed.component === ALL ? null : parsed.component,
        status: parsed.status,
        note: parsed.note.trim() || null,
        starts: parsed.starts,
        ends: parsed.ends,
      });
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.status.overrides() });
      addToast(t('pages.admin.status.tabs.overrides.page.toast.created', {}), 'success');
    },
  });

  const fields: FieldDef<OverrideFormValues>[] = [
    {
      type: 'select',
      name: 'component',
      label: t('pages.admin.status.tabs.overrides.page.column.component', {}),
      options: [
        { value: ALL, label: t('pages.admin.status.label.allComponents', {}) },
        ...components.map((component) => ({ value: component.uuid, label: component.label })),
      ],
      required: true,
      colSpan: 'full',
      props: { allowDeselect: false, searchable: true },
    },
    {
      type: 'select',
      name: 'status',
      label: t('common.table.columns.status', {}),
      options: componentStatusOptions(),
      required: true,
      colSpan: 'full',
      props: { allowDeselect: false },
    },
    {
      type: 'textarea',
      name: 'note',
      label: t('common.table.columns.notes', {}),
      description: t('pages.admin.status.tabs.overrides.page.form.noteDescription', {}),
      rows: 2,
      colSpan: 'full',
    },
    {
      type: 'date',
      name: 'starts',
      label: t('pages.admin.status.tabs.overrides.page.column.starts', {}),
      description: t('pages.admin.status.tabs.overrides.page.form.startsDescription', {}),
      props: { clearable: true },
    },
    {
      type: 'date',
      name: 'ends',
      label: t('pages.admin.status.tabs.overrides.page.column.ends', {}),
      description: t('pages.admin.status.tabs.overrides.page.form.endsDescription', {}),
      props: { clearable: true },
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
      title={t('pages.admin.status.tabs.overrides.page.modal.create.title', {})}
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
