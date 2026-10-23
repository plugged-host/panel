// plugged.host: Admin › Settings › noprofit.space
import { z } from 'zod';
import type { FieldDef } from '@/elements/form-engine/index.ts';
import { adminSettingsNoprofitSchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type NoprofitFormValues = z.infer<typeof adminSettingsNoprofitSchema>;

export const noprofitEmptyFormValues: NoprofitFormValues = {
  enabled: false,
  accountsUrl: 'https://accounts.noprofit.space',
  ssoClient: '',
  ssoAudience: null,
  ssoCallbackUrl: null,
  adminsOnly: false,
  linkByEmail: true,
  ssoSecret: '',
  apiSecret: '',
};

// The secrets are never sent to the browser: an empty field keeps the stored one.
export const noprofitToFormValues = (noprofit: NoprofitFormValues): Partial<NoprofitFormValues> => ({
  ...noprofit,
  ssoSecret: '',
  apiSecret: '',
});

export function useNoprofitFormFields(stored: {
  ssoSecret: boolean;
  apiSecret: boolean;
}): FieldDef<NoprofitFormValues>[] {
  const { t } = useTranslations();

  const secretDescription = (isStored: boolean) =>
    isStored
      ? t('pages.admin.settings.tabs.noprofit.page.form.secretStored', {})
      : t('pages.admin.settings.tabs.noprofit.page.form.secretMissing', {});

  return [
    {
      type: 'switch',
      name: 'enabled',
      label: t('pages.admin.settings.tabs.noprofit.page.form.enabled', {}),
      description: t('pages.admin.settings.tabs.noprofit.page.form.enabledDescription', {}),
    },
    {
      type: 'text',
      name: 'accountsUrl',
      label: t('pages.admin.settings.tabs.noprofit.page.form.accountsUrl', {}),
      required: true,
    },
    {
      type: 'text',
      name: 'ssoClient',
      label: t('pages.admin.settings.tabs.noprofit.page.form.ssoClient', {}),
      description: t('pages.admin.settings.tabs.noprofit.page.form.ssoClientDescription', {}),
    },
    {
      type: 'password',
      name: 'ssoSecret',
      label: t('pages.admin.settings.tabs.noprofit.page.form.ssoSecret', {}),
      description: secretDescription(stored.ssoSecret),
      props: { autoComplete: 'new-password' },
    },
    {
      type: 'password',
      name: 'apiSecret',
      label: t('pages.admin.settings.tabs.noprofit.page.form.apiSecret', {}),
      description: secretDescription(stored.apiSecret),
      props: { autoComplete: 'new-password' },
    },
    {
      type: 'switch',
      name: 'adminsOnly',
      label: t('pages.admin.settings.tabs.noprofit.page.form.adminsOnly', {}),
      description: t('pages.admin.settings.tabs.noprofit.page.form.adminsOnlyDescription', {}),
    },
    {
      type: 'switch',
      name: 'linkByEmail',
      label: t('pages.admin.settings.tabs.noprofit.page.form.linkByEmail', {}),
      description: t('pages.admin.settings.tabs.noprofit.page.form.linkByEmailDescription', {}),
    },
    {
      type: 'text',
      name: 'ssoAudience',
      label: t('pages.admin.settings.tabs.noprofit.page.form.ssoAudience', {}),
      description: t('pages.admin.settings.tabs.noprofit.page.form.ssoAudienceDescription', {}),
      advanced: true,
    },
    {
      type: 'text',
      name: 'ssoCallbackUrl',
      label: t('pages.admin.settings.tabs.noprofit.page.form.ssoCallbackUrl', {}),
      description: t('pages.admin.settings.tabs.noprofit.page.form.ssoCallbackUrlDescription', {}),
      advanced: true,
    },
  ];
}
