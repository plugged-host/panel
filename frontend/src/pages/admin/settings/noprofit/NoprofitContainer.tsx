// plugged.host: Admin › Settings › noprofit.space
import { useState } from 'react';
import { z } from 'zod';
import updateNoprofitSettings from '@/api/admin/settings/updateNoprofitSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsNoprofitSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';
import { noprofitEmptyFormValues, noprofitToFormValues, useNoprofitFormFields } from './noprofitFormValues.tsx';

type NoprofitFormValues = z.infer<typeof adminSettingsNoprofitSchema>;

export default function NoprofitContainer() {
  const { t } = useTranslations();
  const noprofit = useAdminStore((state) => state.noprofit);
  const [stored, setStored] = useState(() => ({
    ssoSecret: noprofit.ssoSecret !== null,
    apiSecret: noprofit.apiSecret !== null,
  }));

  const form = useFormEngine<NoprofitFormValues>('admin.settings.noprofit', {
    schema: adminSettingsNoprofitSchema,
    initialValues: noprofitEmptyFormValues,
  });

  useHydrateForm(form, noprofit, noprofitToFormValues);
  useFormDraft(form, 'noprofit');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsNoprofitSchema,
    storeKey: 'noprofit',
    // A typed secret is stored from now on.
    update: (values) =>
      updateNoprofitSettings(values).then(() =>
        setStored((stored) => ({
          ssoSecret: stored.ssoSecret || !!values.ssoSecret,
          apiSecret: stored.apiSecret || !!values.apiSecret,
        })),
      ),
    successMessage: t('pages.admin.settings.tabs.noprofit.page.toast.updated', {}),
  });

  const fields = useNoprofitFormFields(stored);

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.noprofit.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
