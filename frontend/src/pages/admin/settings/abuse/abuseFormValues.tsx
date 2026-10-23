// plugged.host: Admin › Settings › Abuse
import { z } from 'zod';
import type { FieldDef } from '@/elements/form-engine/index.ts';
import { adminSettingsAbuseSchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type AbuseFormValues = z.infer<typeof adminSettingsAbuseSchema>;

export const abuseEmptyFormValues: AbuseFormValues = {
  mode: 'report',
  alertWebhook: '',
  cpuPercent: 90,
  cpuMinutes: 20,
  deleteAfterHours: 24,
};

// The webhook is never sent to the browser: an empty field keeps the stored one.
export const abuseToFormValues = (abuse: AbuseFormValues): Partial<AbuseFormValues> => ({
  ...abuse,
  alertWebhook: '',
});

export function useAbuseFormFields(
  webhookStored: boolean,
  removeWebhook: FieldDef<AbuseFormValues> | null,
): FieldDef<AbuseFormValues>[] {
  const { t } = useTranslations();

  return [
    {
      type: 'select',
      name: 'mode',
      label: t('pages.admin.settings.tabs.abuse.page.form.mode', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.modeDescription', {}),
      options: [
        { value: 'suspend', label: t('pages.admin.settings.tabs.abuse.page.form.modeSuspend', {}) },
        { value: 'report', label: t('pages.admin.settings.tabs.abuse.page.form.modeReport', {}) },
        { value: 'off', label: t('pages.admin.settings.tabs.abuse.page.form.modeOff', {}) },
      ],
      required: true,
      props: { allowDeselect: false },
    },
    {
      type: 'number',
      name: 'deleteAfterHours',
      label: t('pages.admin.settings.tabs.abuse.page.form.deleteAfterHours', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.deleteAfterHoursDescription', {}),
      required: true,
      props: { min: 0, max: 8760 },
    },
    {
      type: 'password',
      name: 'alertWebhook',
      label: t('pages.admin.settings.tabs.abuse.page.form.alertWebhook', {}),
      description: webhookStored
        ? t('pages.admin.settings.tabs.abuse.page.form.alertWebhookStored', {})
        : t('pages.admin.settings.tabs.abuse.page.form.alertWebhookDescription', {}),
      props: { autoComplete: 'new-password', placeholder: 'https://discord.com/api/webhooks/…' },
    },
    ...(removeWebhook ? [removeWebhook] : []),
    {
      type: 'number',
      name: 'cpuPercent',
      label: t('pages.admin.settings.tabs.abuse.page.form.cpuPercent', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.cpuPercentDescription', {}),
      required: true,
      advanced: true,
      props: { min: 50, max: 100, suffix: '%' },
    },
    {
      type: 'number',
      name: 'cpuMinutes',
      label: t('pages.admin.settings.tabs.abuse.page.form.cpuMinutes', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.cpuMinutesDescription', {}),
      required: true,
      advanced: true,
      props: { min: 5, max: 1440 },
    },
  ];
}
