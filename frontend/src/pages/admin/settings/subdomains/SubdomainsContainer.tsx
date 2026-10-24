// plugged.host: Admin › Settings › Hosting › Subdomains (docs/features/subdomains.md)
import { useState } from 'react';
import { z } from 'zod';
import updateSubdomainsSettings from '@/api/admin/settings/updateSubdomainsSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsSubdomainsSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type SubdomainsValues = z.infer<typeof adminSettingsSubdomainsSchema>;

const subdomainsEmptyFormValues: SubdomainsValues = {
  enabled: false,
  cloudflareToken: '',
  defaultLimit: 3,
  costFirst: 1,
  costSecond: 3,
  costThird: 8,
  costMore: 15,
  blockedNames: '',
};

export default function SubdomainsContainer() {
  const { t } = useTranslations();
  const subdomains = useAdminStore((state) => state.subdomains);
  // a stored token comes back as a placeholder
  const [tokenStored, setTokenStored] = useState(() => !!subdomains.cloudflareToken);
  const page = 'pages.admin.settings.tabs.subdomains.page';

  const form = useFormEngine<SubdomainsValues>('admin.settings.subdomains', {
    schema: adminSettingsSubdomainsSchema,
    initialValues: subdomainsEmptyFormValues,
  });

  // the token never comes back: the field stays empty and keeps it
  useHydrateForm(form, subdomains, (subdomains) => ({ ...subdomains, cloudflareToken: '' }));
  useFormDraft(form, 'subdomains');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsSubdomainsSchema,
    storeKey: 'subdomains',
    // a typed token is stored from now on
    update: (values) =>
      updateSubdomainsSettings(values).then(() =>
        setTokenStored((stored) => stored || !!values.cloudflareToken.trim()),
      ),
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const fields: FieldDef<SubdomainsValues>[] = [
    {
      type: 'switch',
      name: 'enabled',
      label: t(`${page}.form.enabled`, {}),
      description: t(`${page}.form.enabledDescription`, {}),
    },
    {
      type: 'password',
      name: 'cloudflareToken',
      label: t(`${page}.form.cloudflareToken`, {}),
      description: tokenStored
        ? t(`${page}.form.cloudflareTokenStored`, {})
        : t(`${page}.form.cloudflareTokenMissing`, {}),
      props: { autoComplete: 'new-password' },
    },
    {
      type: 'number',
      name: 'defaultLimit',
      label: t(`${page}.form.defaultLimit`, {}),
      description: t(`${page}.form.defaultLimitDescription`, {}),
      required: true,
      props: { min: 0, max: 100 },
    },
    {
      type: 'divider',
      name: 'costsDivider',
      label: t(`${page}.form.costs`, {}),
    },
    {
      type: 'number',
      name: 'costFirst',
      label: t(`${page}.form.costFirst`, {}),
      description: t(`${page}.form.costsDescription`, {}),
      required: true,
      props: { min: 0 },
    },
    {
      type: 'number',
      name: 'costSecond',
      label: t(`${page}.form.costSecond`, {}),
      required: true,
      props: { min: 0 },
    },
    {
      type: 'number',
      name: 'costThird',
      label: t(`${page}.form.costThird`, {}),
      required: true,
      props: { min: 0 },
    },
    {
      type: 'number',
      name: 'costMore',
      label: t(`${page}.form.costMore`, {}),
      required: true,
      props: { min: 0 },
    },
    {
      type: 'textarea',
      name: 'blockedNames',
      label: t(`${page}.form.blockedNames`, {}),
      description: t(`${page}.form.blockedNamesDescription`, {}),
      rows: 3,
    },
  ];

  return (
    <AdminSubContentContainer title={t(`${page}.title`, {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
