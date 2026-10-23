import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateUpdatesSettings from '@/api/admin/settings/updateUpdatesSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import { FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminSettingsUpdatesSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';
import { updatesEmptyFormValues, updatesToFormValues, useUpdatesFormFields } from './updatesFormValues.tsx';

type UpdatesValues = z.infer<typeof adminSettingsUpdatesSchema>;

export default function UpdatesContainer() {
  const { t } = useTranslations();
  const updates = useAdminStore((state) => state.updates);

  const form = useForm<UpdatesValues>({
    initialValues: updatesEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsUpdatesSchema),
  });

  useHydrateForm(form, updates, updatesToFormValues);
  useFormDraft(form, 'updates');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsUpdatesSchema,
    storeKey: 'updates',
    update: updateUpdatesSettings,
    successMessage: t('pages.admin.settings.tabs.updates.page.toast.updated', {}),
  });

  const fields = useUpdatesFormFields();

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.updates.page.title', {})} titleOrder={2}>
      <Alert mb='md'>
        <Text size='sm'>{t('pages.admin.settings.tabs.updates.page.alert', {}).md()}</Text>
      </Alert>

      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
