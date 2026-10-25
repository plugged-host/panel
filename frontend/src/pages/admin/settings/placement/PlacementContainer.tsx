// plugged.host: Admin › Settings › Hosting › Placement, the automatic deployment pause
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updatePlacementSettings from '@/api/admin/settings/updatePlacementSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsPlacementSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type PlacementValues = z.infer<typeof adminSettingsPlacementSchema>;

const placementEmptyFormValues: PlacementValues = {
  pauseEnabled: true,
  pausePercent: 85,
  smallOffset: 10,
  smallMemoryGb: 34,
  smallThreads: 9,
  unreachableAfter: 3,
  diskPercent: 85,
  portRanges: '30000-50000,25000-30000',
};

export default function PlacementContainer() {
  const { t } = useTranslations();
  const placement = useAdminStore((state) => state.placement);
  const page = 'pages.admin.settings.tabs.placement.page.form';

  const form = useForm<PlacementValues>({
    initialValues: placementEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsPlacementSchema),
  });

  useHydrateForm(form, placement, (values: PlacementValues) => values);
  useFormDraft(form, 'placement');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsPlacementSchema,
    storeKey: 'placement',
    update: updatePlacementSettings,
    successMessage: t('pages.admin.settings.tabs.placement.page.toast.updated', {}),
  });

  const fields: FieldDef<PlacementValues>[] = [
    {
      type: 'switch',
      name: 'pauseEnabled',
      label: t(`${page}.pauseEnabled`, {}),
      description: t(`${page}.pauseEnabledDescription`, {}),
    },
    {
      type: 'number',
      name: 'pausePercent',
      label: t(`${page}.pausePercent`, {}),
      description: t(`${page}.pausePercentDescription`, {}),
      required: true,
      props: { min: 10, max: 100 },
    },
    {
      type: 'number',
      name: 'unreachableAfter',
      label: t(`${page}.unreachableAfter`, {}),
      description: t(`${page}.unreachableAfterDescription`, {}),
      required: true,
      props: { min: 1, max: 60 },
    },
    {
      type: 'number',
      name: 'diskPercent',
      label: t(`${page}.diskPercent`, {}),
      description: t(`${page}.diskPercentDescription`, {}),
      required: true,
      props: { min: 0, max: 100 },
    },
    {
      type: 'divider',
      name: 'smallDivider',
      label: t(`${page}.small`, {}),
    },
    {
      type: 'number',
      name: 'smallOffset',
      label: t(`${page}.smallOffset`, {}),
      description: t(`${page}.smallOffsetDescription`, {}),
      required: true,
      props: { min: 0, max: 50 },
    },
    {
      type: 'number',
      name: 'smallMemoryGb',
      label: t(`${page}.smallMemoryGb`, {}),
      required: true,
      props: { min: 0, max: 4096 },
    },
    {
      type: 'number',
      name: 'smallThreads',
      label: t(`${page}.smallThreads`, {}),
      required: true,
      props: { min: 0, max: 1024 },
    },
    {
      type: 'divider',
      name: 'portsDivider',
      label: t(`${page}.ports`, {}),
    },
    {
      type: 'text',
      name: 'portRanges',
      label: t(`${page}.portRanges`, {}),
      description: t(`${page}.portRangesDescription`, {}),
      required: true,
    },
  ];

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.placement.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
