import { z } from 'zod';
import type { FieldDef } from '@/elements/form-engine/index.ts';
import { adminSettingsUpdatesSchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type UpdatesFormValues = z.infer<typeof adminSettingsUpdatesSchema>;

export const updatesEmptyFormValues: UpdatesFormValues = {
  wingsReleaseToken: null,
  dbAgentReleaseToken: null,
};

export const updatesToFormValues = (updates: UpdatesFormValues): Partial<UpdatesFormValues> => ({ ...updates });

export function useUpdatesFormFields(): FieldDef<UpdatesFormValues>[] {
  const { t } = useTranslations();

  return [
    {
      type: 'password',
      name: 'wingsReleaseToken',
      label: t('pages.admin.settings.tabs.updates.page.form.wingsReleaseToken', {}),
    },
    {
      type: 'password',
      name: 'dbAgentReleaseToken',
      label: t('pages.admin.settings.tabs.updates.page.form.dbAgentReleaseToken', {}),
    },
  ];
}
