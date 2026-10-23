// plugged.host: Admin › Status › Components: whether and where the status page lists a component.
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import updateStatusComponent from '@/api/admin/status/updateStatusComponent.ts';
import Button from '@/elements/buttons/Button.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminStatusComponent } from '@/lib/schemas/admin/status.ts';
import { useModalForm } from '@/plugins/form/useModalForm.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const componentFormSchema = z.object({
  label: z.string().trim().min(1).max(255),
  published: z.boolean(),
  sort: z.number().int().min(0).max(100000),
});

type ComponentFormValues = z.infer<typeof componentFormSchema>;

export default function ComponentEditModal({
  component,
  opened,
  onClose,
  ...props
}: ModalProps & { component: AdminStatusComponent }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();

  const initialValues: ComponentFormValues = {
    label: component.label,
    published: component.published,
    sort: component.sort,
  };
  // A node's label is its name.
  const isNode = component.kind === 'node';

  const { form, handleClose, handleSubmit, loading, isDirty } = useModalForm<ComponentFormValues>({
    schema: componentFormSchema,
    initialValues,
    opened,
    onClose,
    hydrate: () => initialValues,
    onSubmit: async (values) => {
      await updateStatusComponent(component.uuid, {
        published: values.published,
        sort: values.sort,
        label: isNode ? undefined : values.label,
      });
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.status.components() });
      addToast(t('pages.admin.status.tabs.components.page.toast.updated', {}), 'success');
    },
  });

  const fields: FieldDef<ComponentFormValues>[] = [
    {
      type: 'text',
      name: 'label',
      label: t('common.table.columns.name', {}),
      description: isNode ? t('pages.admin.status.tabs.components.page.form.labelNode', {}) : undefined,
      required: true,
      colSpan: 'full',
      props: { disabled: isNode },
    },
    {
      type: 'switch',
      name: 'published',
      label: t('pages.admin.status.tabs.components.page.column.published', {}),
      description: t('pages.admin.status.tabs.components.page.form.publishedDescription', {}),
      colSpan: 'full',
    },
    {
      type: 'number',
      name: 'sort',
      label: t('pages.admin.status.tabs.components.page.column.sort', {}),
      description: t('pages.admin.status.tabs.components.page.form.sortDescription', {}),
      required: true,
      colSpan: 'full',
      props: { min: 0, max: 100000 },
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
      title={t('pages.admin.status.tabs.components.page.modal.edit.title', { label: component.label })}
    >
      <Stack gap='md'>
        <FormEngine form={form} fields={fields} />

        <ModalFooter>
          <Button type='submit' disabled={!form.isValid()} loading={loading}>
            {t('common.button.save', {})}
          </Button>
          <Button variant='default' onClick={handleClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </FormModal>
  );
}
