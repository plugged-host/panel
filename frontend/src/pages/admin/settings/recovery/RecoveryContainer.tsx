// plugged.host: Admin › Settings › Hosting › Recovery (docs/features/backups-page.md, failover.md)
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateRecoverySettings from '@/api/admin/settings/updateRecoverySettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsRecoverySchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type RecoveryValues = z.infer<typeof adminSettingsRecoverySchema>;

const recoveryEmptyFormValues: RecoveryValues = {
  deletedHoldDays: 7,
  failoverMode: 'manual',
  failoverAfter: 10,
  failoverParallel: 3,
};

export default function RecoveryContainer() {
  const { t } = useTranslations();
  const recovery = useAdminStore((state) => state.recovery);
  const page = 'pages.admin.settings.tabs.recovery.page.form';

  const form = useForm<RecoveryValues>({
    initialValues: recoveryEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsRecoverySchema),
  });

  useHydrateForm(form, recovery, (values: RecoveryValues) => values);
  useFormDraft(form, 'recovery');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsRecoverySchema,
    storeKey: 'recovery',
    update: updateRecoverySettings,
    successMessage: t('pages.admin.settings.tabs.recovery.page.toast.updated', {}),
  });

  const fields: FieldDef<RecoveryValues>[] = [
    {
      type: 'number',
      name: 'deletedHoldDays',
      label: t(`${page}.deletedHoldDays`, {}),
      description: t(`${page}.deletedHoldDaysDescription`, {}),
      required: true,
      props: { min: 0, max: 90 },
    },
    {
      type: 'select',
      name: 'failoverMode',
      label: t(`${page}.failoverMode`, {}),
      description: t(`${page}.failoverModeDescription`, {}),
      options: [
        { value: 'off', label: t(`${page}.failoverModeOff`, {}) },
        { value: 'manual', label: t(`${page}.failoverModeManual`, {}) },
        { value: 'auto', label: t(`${page}.failoverModeAuto`, {}) },
      ],
      required: true,
      props: { allowDeselect: false },
    },
    {
      type: 'number',
      name: 'failoverAfter',
      label: t(`${page}.failoverAfter`, {}),
      description: t(`${page}.failoverAfterDescription`, {}),
      required: true,
      props: { min: 3, max: 1440 },
    },
    {
      type: 'number',
      name: 'failoverParallel',
      label: t(`${page}.failoverParallel`, {}),
      description: t(`${page}.failoverParallelDescription`, {}),
      required: true,
      props: { min: 1, max: 20 },
    },
  ];

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.recovery.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
