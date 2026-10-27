// plugged.host: Admin › Settings › Security › Abuse
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
  standingLimitedAt: 2,
  standingLimitedDays: 7,
  standingVeryLimitedAt: 3,
  standingVeryLimitedDays: 30,
  standingSuspendedAt: 4,
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
    // the panel's own account standing (docs/features/standing.md)
    {
      type: 'number',
      name: 'standingLimitedAt',
      label: t('pages.admin.settings.tabs.abuse.page.form.standingLimitedAt', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.standingLimitedAtDescription', {}),
      required: true,
      advanced: true,
      props: { min: 1, max: 100 },
    },
    {
      type: 'number',
      name: 'standingLimitedDays',
      label: t('pages.admin.settings.tabs.abuse.page.form.standingLimitedDays', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.standingLimitedDaysDescription', {}),
      required: true,
      advanced: true,
      props: { min: 1, max: 3650 },
    },
    {
      type: 'number',
      name: 'standingVeryLimitedAt',
      label: t('pages.admin.settings.tabs.abuse.page.form.standingVeryLimitedAt', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.standingVeryLimitedAtDescription', {}),
      required: true,
      advanced: true,
      props: { min: 1, max: 100 },
    },
    {
      type: 'number',
      name: 'standingVeryLimitedDays',
      label: t('pages.admin.settings.tabs.abuse.page.form.standingVeryLimitedDays', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.standingVeryLimitedDaysDescription', {}),
      required: true,
      advanced: true,
      props: { min: 1, max: 3650 },
    },
    {
      type: 'number',
      name: 'standingSuspendedAt',
      label: t('pages.admin.settings.tabs.abuse.page.form.standingSuspendedAt', {}),
      description: t('pages.admin.settings.tabs.abuse.page.form.standingSuspendedAtDescription', {}),
      required: true,
      advanced: true,
      props: { min: 1, max: 100 },
    },
  ];
}
