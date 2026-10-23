// plugged.host: Admin › Settings › Welcome Tour (docs/features/welcome-tour.md)
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateTourSettings from '@/api/admin/settings/updateTourSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsTourSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type TourValues = z.infer<typeof adminSettingsTourSchema>;

const tourEmptyFormValues: TourValues = {
  enabled: false,
  version: 1,
};

export default function TourContainer() {
  const { t } = useTranslations();
  const tour = useAdminStore((state) => state.tour);
  const page = 'pages.admin.settings.tabs.tour.page.form';

  const form = useForm<TourValues>({
    initialValues: tourEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsTourSchema),
  });

  useHydrateForm(form, tour, (values: TourValues) => values);
  useFormDraft(form, 'tour');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsTourSchema,
    storeKey: 'tour',
    update: updateTourSettings,
    successMessage: t('pages.admin.settings.tabs.tour.page.toast.updated', {}),
  });

  const fields: FieldDef<TourValues>[] = [
    {
      type: 'switch',
      name: 'enabled',
      label: t(`${page}.enabled`, {}),
      description: t(`${page}.enabledDescription`, {}),
    },
    {
      type: 'number',
      name: 'version',
      label: t(`${page}.version`, {}),
      description: t(`${page}.versionDescription`, {}),
      required: true,
      props: { min: 1, max: 1000 },
    },
  ];

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.tour.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
