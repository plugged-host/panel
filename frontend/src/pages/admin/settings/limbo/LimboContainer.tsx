// plugged.host: Admin › Settings › Hosting › Limbo (docs/features/limbo.md)
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateLimboSettings from '@/api/admin/settings/updateLimboSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsLimboSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type LimboValues = z.infer<typeof adminSettingsLimboSchema>;

const limboEmptyFormValues: LimboValues = {
  enabled: false,
  stopMinutes: 2880,
  parkMinutes: 4320,
  maxPerRun: 3,
  costEarly: 5,
  costLate: 10,
  costWindowHours: 72,
  exemptMembers: true,
};

export default function LimboContainer() {
  const { t } = useTranslations();
  const limbo = useAdminStore((state) => state.limbo);
  const page = 'pages.admin.settings.tabs.limbo.page.form';

  const form = useForm<LimboValues>({
    initialValues: limboEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsLimboSchema),
  });

  useHydrateForm(form, limbo, (values: LimboValues) => values);
  useFormDraft(form, 'limbo');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsLimboSchema,
    storeKey: 'limbo',
    update: updateLimboSettings,
    successMessage: t('pages.admin.settings.tabs.limbo.page.toast.updated', {}),
  });

  const fields: FieldDef<LimboValues>[] = [
    {
      type: 'switch',
      name: 'enabled',
      label: t(`${page}.enabled`, {}),
      description: t(`${page}.enabledDescription`, {}),
    },
    {
      type: 'number',
      name: 'stopMinutes',
      label: t(`${page}.stopMinutes`, {}),
      description: t(`${page}.stopMinutesDescription`, {}),
      required: true,
      props: { min: 1, max: 525_600 },
    },
    {
      type: 'number',
      name: 'parkMinutes',
      label: t(`${page}.parkMinutes`, {}),
      description: t(`${page}.parkMinutesDescription`, {}),
      required: true,
      props: { min: 1, max: 525_600 },
    },
    {
      type: 'number',
      name: 'maxPerRun',
      label: t(`${page}.maxPerRun`, {}),
      description: t(`${page}.maxPerRunDescription`, {}),
      required: true,
      props: { min: 1, max: 100 },
    },
    {
      type: 'switch',
      name: 'exemptMembers',
      label: t(`${page}.exemptMembers`, {}),
      description: t(`${page}.exemptMembersDescription`, {}),
    },
    {
      type: 'divider',
      name: 'costDivider',
      label: t(`${page}.cost`, {}),
    },
    {
      type: 'number',
      name: 'costEarly',
      label: t(`${page}.costEarly`, {}),
      description: t(`${page}.costEarlyDescription`, {}),
      required: true,
      props: { min: 0, max: 100000 },
    },
    {
      type: 'number',
      name: 'costLate',
      label: t(`${page}.costLate`, {}),
      description: t(`${page}.costLateDescription`, {}),
      required: true,
      props: { min: 0, max: 100000 },
    },
    {
      type: 'number',
      name: 'costWindowHours',
      label: t(`${page}.costWindowHours`, {}),
      description: t(`${page}.costWindowHoursDescription`, {}),
      required: true,
      props: { min: 0, max: 8760 },
    },
  ];

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.limbo.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
