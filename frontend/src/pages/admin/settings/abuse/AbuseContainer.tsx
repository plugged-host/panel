// plugged.host: Admin › Settings › Security › Abuse
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useState } from 'react';
import { z } from 'zod';
import removeAbuseAlertWebhook from '@/api/admin/settings/removeAbuseAlertWebhook.ts';
import updateAbuseSettings from '@/api/admin/settings/updateAbuseSettings.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsAbuseSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';
import { abuseEmptyFormValues, abuseToFormValues, useAbuseFormFields } from './abuseFormValues.tsx';

type AbuseValues = z.infer<typeof adminSettingsAbuseSchema>;

export default function AbuseContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const abuse = useAdminStore((state) => state.abuse);
  const [webhookStored, setWebhookStored] = useState(() => abuse.alertWebhook !== null);
  const [removing, setRemoving] = useState(false);

  const form = useForm<AbuseValues>({
    initialValues: abuseEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsAbuseSchema),
  });

  useHydrateForm(form, abuse, abuseToFormValues);
  useFormDraft(form, 'abuse');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsAbuseSchema,
    storeKey: 'abuse',
    // A typed webhook is stored from now on.
    update: (values) =>
      updateAbuseSettings(values).then(() => setWebhookStored((stored) => stored || !!values.alertWebhook)),
    successMessage: t('pages.admin.settings.tabs.abuse.page.toast.updated', {}),
  });

  const removeWebhook = () => {
    setRemoving(true);
    removeAbuseAlertWebhook()
      .then(() => {
        setWebhookStored(false);
        addToast(t('pages.admin.settings.tabs.abuse.page.toast.webhookRemoved', {}), 'success');
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setRemoving(false));
  };

  const fields = useAbuseFormFields(
    webhookStored,
    webhookStored
      ? {
          type: 'custom',
          name: 'removeAlertWebhook',
          colSpan: 'full',
          render: () => (
            <Group>
              <Button variant='default' size='xs' loading={removing} onClick={removeWebhook}>
                {t('pages.admin.settings.tabs.abuse.page.button.removeWebhook', {})}
              </Button>
            </Group>
          ),
        }
      : null,
  );

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.abuse.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
