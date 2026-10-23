// plugged.host: Admin › Settings › General › Status Page
import { z } from 'zod';
import type { FieldDef } from '@/elements/form-engine/index.ts';
import { adminSettingsStatusSchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type StatusFormValues = z.infer<typeof adminSettingsStatusSchema>;

export const statusEmptyFormValues: StatusFormValues = {
  enabled: false,
  siteUrl: null,
  siteHost: null,
  panelUrl: null,
  pageUrl: null,
  historyDays: 90,
  cpuThreshold: 95,
};

export const statusToFormValues = (status: StatusFormValues): Partial<StatusFormValues> => ({ ...status });

export function useStatusFormFields(): FieldDef<StatusFormValues>[] {
  const { t } = useTranslations();

  return [
    {
      type: 'switch',
      name: 'enabled',
      label: t('pages.admin.settings.tabs.status.page.form.enabled', {}),
      description: t('pages.admin.settings.tabs.status.page.form.enabledDescription', {}),
    },
    {
      type: 'text',
      name: 'siteUrl',
      label: t('pages.admin.settings.tabs.status.page.form.siteUrl', {}),
      description: t('pages.admin.settings.tabs.status.page.form.siteUrlDescription', {}),
      props: { placeholder: 'https://example.com' },
    },
    {
      type: 'text',
      name: 'panelUrl',
      label: t('pages.admin.settings.tabs.status.page.form.panelUrl', {}),
      description: t('pages.admin.settings.tabs.status.page.form.panelUrlDescription', {}),
      props: { placeholder: 'https://panel.example.com/api/settings' },
    },
    {
      type: 'text',
      name: 'pageUrl',
      label: t('pages.admin.settings.tabs.status.page.form.pageUrl', {}),
      description: t('pages.admin.settings.tabs.status.page.form.pageUrlDescription', {}),
      props: { placeholder: 'https://status.example.com' },
    },
    {
      type: 'number',
      name: 'cpuThreshold',
      label: t('pages.admin.settings.tabs.status.page.form.cpuThreshold', {}),
      description: t('pages.admin.settings.tabs.status.page.form.cpuThresholdDescription', {}),
      required: true,
      props: { min: 1, max: 100, suffix: '%' },
    },
    {
      type: 'number',
      name: 'historyDays',
      label: t('pages.admin.settings.tabs.status.page.form.historyDays', {}),
      description: t('pages.admin.settings.tabs.status.page.form.historyDaysDescription', {}),
      required: true,
      props: { min: 1, max: 365 },
    },
    {
      type: 'text',
      name: 'siteHost',
      label: t('pages.admin.settings.tabs.status.page.form.siteHost', {}),
      description: t('pages.admin.settings.tabs.status.page.form.siteHostDescription', {}),
      advanced: true,
    },
  ];
}
