// plugged.host: Admin › Settings › General › Status Page
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateStatusSettings from '@/api/admin/settings/updateStatusSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsStatusSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';
import { statusEmptyFormValues, statusToFormValues, useStatusFormFields } from './statusFormValues.tsx';

type StatusValues = z.infer<typeof adminSettingsStatusSchema>;

export default function StatusContainer() {
  const { t } = useTranslations();
  const status = useAdminStore((state) => state.status);

  const form = useForm<StatusValues>({
    initialValues: statusEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsStatusSchema),
  });

  useHydrateForm(form, status, statusToFormValues);
  useFormDraft(form, 'status');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsStatusSchema,
    storeKey: 'status',
    update: updateStatusSettings,
    successMessage: t('pages.admin.settings.tabs.status.page.toast.updated', {}),
  });

  const fields = useStatusFormFields();

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.status.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
