// plugged.host: Admin › Settings › Games, what players may create and how much
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateGamesSettings from '@/api/admin/settings/updateGamesSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsGamesSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';
import { gamesEmptyFormValues, gamesToFormValues, useGamesFormFields } from './gamesFormValues.tsx';

type GamesValues = z.infer<typeof adminSettingsGamesSchema>;

export default function GamesContainer() {
  const { t } = useTranslations();
  const games = useAdminStore((state) => state.games);
  const noprofit = useAdminStore((state) => state.noprofit);

  const form = useForm<GamesValues>({
    initialValues: gamesEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsGamesSchema),
  });

  useHydrateForm(form, games, gamesToFormValues);
  useFormDraft(form, 'games');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsGamesSchema,
    storeKey: 'games',
    update: updateGamesSettings,
    successMessage: t('pages.admin.settings.tabs.games.page.toast.updated', {}),
  });

  const fields = useGamesFormFields(form, noprofit.enabled);

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.games.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
