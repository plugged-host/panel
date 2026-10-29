// plugged.host: Admin › Settings › Economy › Server Upgrades: each upgrade's switch, price and term
// (docs/features/server-upgrades.md)
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateUpgradesSettings from '@/api/admin/settings/updateUpgradesSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsUpgradesSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type UpgradesValues = z.infer<typeof adminSettingsUpgradesSchema>;

const upgradesEmptyFormValues: UpgradesValues = {
  keepAwakeEnabled: true,
  keepAwakeTokens: 3,
  keepAwakeDays: 7,
  boostEnabled: true,
  boostTokens: 3,
  boostHours: 24,
  boostMemory: 2048,
  boostCpu: 100,
  extraAllocationEnabled: true,
  extraAllocationTokens: 2,
  extraAllocationDays: 30,
  extraAllocationMax: 3,
  extraDatabaseEnabled: true,
  extraDatabaseTokens: 2,
  extraDatabaseDays: 30,
  extraDatabaseMax: 3,
  extraBackupEnabled: true,
  extraBackupTokens: 2,
  extraBackupDays: 30,
  extraBackupMax: 3,
  shortSubdomainEnabled: true,
  shortSubdomainThree: 15,
  shortSubdomainTwo: 30,
  cardColorEnabled: true,
  cardColorTokens: 2,
};

export default function UpgradesContainer() {
  const { t } = useTranslations();
  const upgrades = useAdminStore((state) => state.upgrades);
  const page = 'pages.admin.settings.tabs.upgrades.page.form';

  const form = useForm<UpgradesValues>({
    initialValues: upgradesEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsUpgradesSchema),
  });

  useHydrateForm(form, upgrades, (values: UpgradesValues) => values);
  useFormDraft(form, 'upgrades');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsUpgradesSchema,
    storeKey: 'upgrades',
    update: updateUpgradesSettings,
    successMessage: t('pages.admin.settings.tabs.upgrades.page.toast.updated', {}),
  });

  const tokens = (name: keyof UpgradesValues, max = 100000): FieldDef<UpgradesValues> => ({
    type: 'number',
    name,
    label: t(`${page}.tokens`, {}),
    required: true,
    props: { min: 0, max },
  });
  const days = (name: keyof UpgradesValues): FieldDef<UpgradesValues> => ({
    type: 'number',
    name,
    label: t(`${page}.days`, {}),
    required: true,
    props: { min: 1, max: 365 },
  });
  const most = (name: keyof UpgradesValues): FieldDef<UpgradesValues> => ({
    type: 'number',
    name,
    label: t(`${page}.max`, {}),
    description: t(`${page}.maxDescription`, {}),
    required: true,
    props: { min: 1, max: 50 },
  });

  const fields: FieldDef<UpgradesValues>[] = [
    { type: 'divider', name: 'keepAwakeDivider', label: t(`${page}.keepAwake`, {}) },
    {
      type: 'switch',
      name: 'keepAwakeEnabled',
      label: t(`${page}.keepAwakeEnabled`, {}),
      description: t(`${page}.keepAwakeEnabledDescription`, {}),
    },
    tokens('keepAwakeTokens'),
    days('keepAwakeDays'),

    { type: 'divider', name: 'boostDivider', label: t(`${page}.boost`, {}) },
    {
      type: 'switch',
      name: 'boostEnabled',
      label: t(`${page}.boostEnabled`, {}),
      description: t(`${page}.boostEnabledDescription`, {}),
    },
    tokens('boostTokens'),
    {
      type: 'number',
      name: 'boostHours',
      label: t(`${page}.hours`, {}),
      required: true,
      props: { min: 1, max: 720 },
    },
    {
      type: 'number',
      name: 'boostMemory',
      label: t(`${page}.boostMemory`, {}),
      required: true,
      props: { min: 0, max: 1048576 },
    },
    {
      type: 'number',
      name: 'boostCpu',
      label: t(`${page}.boostCpu`, {}),
      description: t(`${page}.boostCpuDescription`, {}),
      required: true,
      props: { min: 0, max: 6400 },
    },

    { type: 'divider', name: 'extraAllocationDivider', label: t(`${page}.extraAllocation`, {}) },
    {
      type: 'switch',
      name: 'extraAllocationEnabled',
      label: t(`${page}.extraAllocationEnabled`, {}),
      description: t(`${page}.extraDescription`, {}),
    },
    tokens('extraAllocationTokens'),
    days('extraAllocationDays'),
    most('extraAllocationMax'),

    { type: 'divider', name: 'extraDatabaseDivider', label: t(`${page}.extraDatabase`, {}) },
    {
      type: 'switch',
      name: 'extraDatabaseEnabled',
      label: t(`${page}.extraDatabaseEnabled`, {}),
      description: t(`${page}.extraDescription`, {}),
    },
    tokens('extraDatabaseTokens'),
    days('extraDatabaseDays'),
    most('extraDatabaseMax'),

    { type: 'divider', name: 'extraBackupDivider', label: t(`${page}.extraBackup`, {}) },
    {
      type: 'switch',
      name: 'extraBackupEnabled',
      label: t(`${page}.extraBackupEnabled`, {}),
      description: t(`${page}.extraDescription`, {}),
    },
    tokens('extraBackupTokens'),
    days('extraBackupDays'),
    most('extraBackupMax'),

    { type: 'divider', name: 'shortSubdomainDivider', label: t(`${page}.shortSubdomain`, {}) },
    {
      type: 'switch',
      name: 'shortSubdomainEnabled',
      label: t(`${page}.shortSubdomainEnabled`, {}),
      description: t(`${page}.shortSubdomainEnabledDescription`, {}),
    },
    {
      type: 'number',
      name: 'shortSubdomainThree',
      label: t(`${page}.shortSubdomainThree`, {}),
      required: true,
      props: { min: 0, max: 1000000 },
    },
    {
      type: 'number',
      name: 'shortSubdomainTwo',
      label: t(`${page}.shortSubdomainTwo`, {}),
      required: true,
      props: { min: 0, max: 1000000 },
    },

    { type: 'divider', name: 'cardColorDivider', label: t(`${page}.cardColor`, {}) },
    {
      type: 'switch',
      name: 'cardColorEnabled',
      label: t(`${page}.cardColorEnabled`, {}),
      description: t(`${page}.cardColorEnabledDescription`, {}),
    },
    tokens('cardColorTokens'),
  ];

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.upgrades.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
