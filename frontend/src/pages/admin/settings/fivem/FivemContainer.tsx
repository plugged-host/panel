// plugged.host: Admin › Settings › Hosting › FiveM (docs/features/fivem.md)
import { z } from 'zod';
import updateFivemSettings from '@/api/admin/settings/updateFivemSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsFivemSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type FivemValues = z.infer<typeof adminSettingsFivemSchema>;

const fivemEmptyFormValues: FivemValues = {
  fivemImage: '',
  fivemEnhanced: false,
};

export default function FivemContainer() {
  const { t } = useTranslations();
  const games = useAdminStore((state) => state.games);
  const page = 'pages.admin.settings.tabs.fivem.page';

  const form = useFormEngine<FivemValues>('admin.settings.fivem', {
    schema: adminSettingsFivemSchema,
    initialValues: fivemEmptyFormValues,
  });

  useHydrateForm(form, games, (games) => ({
    fivemImage: games.fivemImage,
    fivemEnhanced: games.fivemEnhanced,
  }));
  useFormDraft(form, 'fivem');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsFivemSchema,
    storeKey: 'games',
    update: updateFivemSettings,
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const fields: FieldDef<FivemValues>[] = [
    {
      type: 'text',
      name: 'fivemImage',
      label: t(`${page}.form.image`, {}),
      description: t(`${page}.form.imageDescription`, {}),
      props: { placeholder: 'ghcr.io/example/fivem:latest' },
    },
    {
      type: 'switch',
      name: 'fivemEnhanced',
      label: t(`${page}.form.enhanced`, {}),
      description: t(`${page}.form.enhancedDescription`, {}),
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
