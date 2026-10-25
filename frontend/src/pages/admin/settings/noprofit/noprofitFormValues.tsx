// plugged.host: Admin › Settings › Accounts › noprofit.space
import { z } from 'zod';
import type { FieldDef } from '@/elements/form-engine/index.ts';
import { adminSettingsNoprofitFormSchema, adminSettingsNoprofitSchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type NoprofitFormValues = z.infer<typeof adminSettingsNoprofitFormSchema>;

export const noprofitEmptyFormValues: NoprofitFormValues = {
  enabled: false,
  accountsUrl: 'https://accounts.noprofit.space',
  ssoClient: '',
  ssoAudience: null,
  ssoCallbackUrl: null,
  adminsOnly: false,
  linkByEmail: true,
  deletedAccountDays: 7,
  ssoSecret: '',
  apiSecret: '',
  neoSecret: '',
  clearNeoSecret: false,
};

// The secrets are never sent to the browser: an empty field keeps the stored one.
export const noprofitToFormValues = (
  noprofit: z.infer<typeof adminSettingsNoprofitSchema>,
): Partial<NoprofitFormValues> => ({
  ...noprofit,
  ssoSecret: '',
  apiSecret: '',
  neoSecret: '',
  clearNeoSecret: false,
});

export function useNoprofitFormFields(stored: {
  ssoSecret: boolean;
  apiSecret: boolean;
  neoSecret: boolean;
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
      type: 'password',
      name: 'neoSecret',
      label: t('pages.admin.settings.tabs.noprofit.page.form.neoSecret', {}),
      description: `${t('pages.admin.settings.tabs.noprofit.page.form.neoSecretDescription', {})} ${secretDescription(stored.neoSecret)}`,
      props: { autoComplete: 'new-password' },
      when: (values) => !values.clearNeoSecret,
    },
    {
      type: 'switch',
      name: 'clearNeoSecret',
      label: t('pages.admin.settings.tabs.noprofit.page.form.clearNeoSecret', {}),
      description: t('pages.admin.settings.tabs.noprofit.page.form.clearNeoSecretDescription', {}),
      when: () => stored.neoSecret,
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
      type: 'number',
      name: 'deletedAccountDays',
      label: t('pages.admin.settings.tabs.noprofit.page.form.deletedAccountDays', {}),
      description: t('pages.admin.settings.tabs.noprofit.page.form.deletedAccountDaysDescription', {}),
      required: true,
      props: { min: 0, max: 365 },
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
