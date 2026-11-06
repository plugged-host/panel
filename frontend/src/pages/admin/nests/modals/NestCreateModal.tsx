import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import createNest from '@/api/admin/nests/createNest.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import { FormEngine } from '@/elements/form-engine/index.ts';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminNestSchema, adminNestUpdateSchema } from '@/lib/schemas/admin/nests.ts';
import { nestEmptyFormValues, useNestFormFields } from '@/pages/admin/nests/nestFormValues.tsx';
import { useModalForm } from '@/plugins/form/useModalForm.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type NestFormValues = z.infer<typeof adminNestUpdateSchema>;

type NestCreateModalProps = ModalProps & {
  onCreated: (nest: z.infer<typeof adminNestSchema>) => void;
};

export default function NestCreateModal({ opened, onClose, onCreated, ...props }: NestCreateModalProps) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const { user } = useAuth();
  const queryClient = useQueryClient();

  const initialValues = { ...nestEmptyFormValues, author: user?.username ?? '' };

  const { form, handleClose, handleSubmit, loading, isDirty } = useModalForm<NestFormValues>({
    formId: 'admin.nests.createOrUpdate',
    schema: adminNestUpdateSchema.unwrap(),
    initialValues,
    opened,
    onClose,
    hydrate: () => initialValues,
    onSubmit: async (values) => {
      const created = await createNest(adminNestUpdateSchema.parse(values));
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.nests.all() });
      addToast(
        t('elements.resource.tooltip.created', { resource: t('pages.admin.nests.resourceName', {}) }),
        'success',
      );
      onCreated(created);
    },
  });

  const fields = useNestFormFields();

  return (
    <FormModal
      {...props}
      opened={opened}
      onClose={handleClose}
      onSubmit={handleSubmit}
      isDirty={isDirty}
      loading={loading}
      title={t('pages.admin.nests.tabs.general.page.titleCreate', {})}
    >
      <Stack gap='md'>
        <FormEngine form={form} fields={fields} />

        <ModalFooter>
          <AdminCan action='nests.create' cantSave>
            <Button type='submit' disabled={!form.isValid()} loading={loading}>
              {t('pages.admin.nests.tabs.general.page.titleCreate', {})}
            </Button>
          </AdminCan>
          <Button variant='default' onClick={handleClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </FormModal>
  );
}
