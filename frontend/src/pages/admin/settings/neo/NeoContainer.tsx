// plugged.host: Admin › Settings › AI › neo, Pelican's neo tab: neo on this panel, its models and its limits (docs/features/neo.md)
import { useEffect, useState } from 'react';
import { z } from 'zod';
import getSettings from '@/api/admin/settings/getSettings.ts';
import updateNeoSettings from '@/api/admin/settings/updateNeoSettings.ts';
import getNeoProviders from '@/api/admin/system/neo/getNeoProviders.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import Text from '@/elements/typography/Text.tsx';
import { neoOnNoprofit } from '@/lib/neo.ts';
import { adminNeoProviderStatusSchema } from '@/lib/schemas/admin/neo.ts';
import { adminSettingsNeoSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import NeoProvidersField from './NeoProvidersField.tsx';
import { type NeoValues, neoEmptyFormValues, neoToFormValues } from './neoFormValues.ts';

type NumberField = {
  [K in keyof NeoValues]: NeoValues[K] extends number ? K : never;
}[keyof NeoValues];

const page = 'pages.admin.settings.tabs.neo.page';

export default function NeoContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const neo = useAdminStore((state) => state.neo);
  const noprofit = useAdminStore((state) => state.noprofit);
  const economy = useAdminStore((state) => state.economy);
  const updateAdminSettings = useAdminStore((state) => state.updateSettings);

  const [loading, setLoading] = useState(false);
  const [statuses, setStatuses] = useState<z.infer<typeof adminNeoProviderStatusSchema>[]>([]);

  const form = useFormEngine<NeoValues>('admin.settings.neo', {
    schema: adminSettingsNeoSchema,
    initialValues: neoEmptyFormValues,
  });

  useHydrateForm(form, neo, neoToFormValues);
  useFormDraft(form, 'neo');

  const loadStatuses = () =>
    getNeoProviders()
      .then(setStatuses)
      .catch((error) => addToast(httpErrorToHuman(error), 'error'));

  useEffect(() => {
    loadStatuses();
  }, []);

  // The stored slots differ from the form's (keys never come back, JSON is text), so the saved
  // section is read back from the panel instead of being copied from the form.
  const submit = () => {
    setLoading(true);

    const submittedValues = form.getValues();

    updateNeoSettings(adminSettingsNeoSchema.parse(submittedValues))
      .then(async () => {
        form.resetDirty(submittedValues);
        addToast(t(`${page}.toast.updated`, {}), 'success');

        const settings = await getSettings();
        updateAdminSettings({ neo: settings.neo });
        await loadStatuses();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  const number = (
    name: NumberField,
    label: string,
    description: string | undefined,
    min: number,
    max?: number,
    colSpan?: 'full',
  ): FieldDef<NeoValues> => ({
    type: 'number',
    name,
    label,
    description,
    required: true,
    colSpan,
    props: { min, max },
  });

  const fields: FieldDef<NeoValues>[] = [
    // Pelican's "neo on this panel"
    { type: 'divider', name: 'panelDivider', label: t(`${page}.form.panel`, {}) },
    {
      type: 'switch',
      name: 'enabled',
      label: t(`${page}.form.enabled`, {}),
      description: t(`${page}.form.enabledDescription`, {}),
      colSpan: 'full',
    },
    number('maxSteps', t(`${page}.form.maxSteps`, {}), t(`${page}.form.maxStepsDescription`, {}), 1, 100),
    number(
      'contextTokens',
      t(`${page}.form.contextTokens`, {}),
      t(`${page}.form.contextTokensDescription`, {}),
      4000,
      400000,
    ),
    number(
      'messagesPerMinute',
      t(`${page}.form.messagesPerMinute`, {}),
      t(`${page}.form.messagesPerMinuteDescription`, {}),
      1,
      600,
    ),
    number(
      'toolsPerMinute',
      t(`${page}.form.toolsPerMinute`, {}),
      t(`${page}.form.toolsPerMinuteDescription`, {}),
      1,
      600,
    ),
    number(
      'retentionDays',
      t(`${page}.form.retentionDays`, {}),
      t(`${page}.form.retentionDaysDescription`, {}),
      1,
      3650,
    ),
    {
      type: 'text',
      name: 'supportUrl',
      label: t(`${page}.form.supportUrl`, {}),
      description: t(`${page}.form.supportUrlDescription`, {}),
      props: { placeholder: 'https://panel.example.com/help' },
    },
    {
      type: 'text',
      name: 'legalEmail',
      label: t(`${page}.form.legalEmail`, {}),
      description: t(`${page}.form.legalEmailDescription`, {}),
      props: { placeholder: 'legal@example.com' },
    },
    {
      type: 'text',
      name: 'privacyEmail',
      label: t(`${page}.form.privacyEmail`, {}),
      description: t(`${page}.form.privacyEmailDescription`, {}),
      props: { placeholder: 'privacy@example.com' },
    },
    {
      type: 'text',
      name: 'providerDisclosure',
      label: t(`${page}.form.providerDisclosure`, {}),
      description: t(`${page}.form.providerDisclosureDescription`, {}),
      colSpan: 'full',
    },
    {
      type: 'switch',
      name: 'alwaysConfirmDestructive',
      label: t(`${page}.form.alwaysConfirmDestructive`, {}),
      description: t(`${page}.form.alwaysConfirmDestructiveDescription`, {}),
      colSpan: 'full',
    },

    // Pelican's "AI providers and limits", then a section per model
    { type: 'divider', name: 'providersDivider', label: t(`${page}.form.providers`, {}) },
    {
      type: 'custom',
      name: 'providersWhere',
      colSpan: 'full',
      render: () => (
        <Text size='sm' c='dimmed'>
          {neoOnNoprofit(noprofit) ? t(`${page}.onNoprofit`, {}) : t(`${page}.onPanel`, {})}
        </Text>
      ),
    },
    {
      type: 'custom',
      name: 'providersField',
      colSpan: 'full',
      render: (form) => <NeoProvidersField form={form} statuses={statuses} />,
    },

    // Pelican's "Usage limits", with the failover timings
    { type: 'divider', name: 'usageDivider', label: t(`${page}.form.usage`, {}) },
    {
      type: 'custom',
      name: 'usageNote',
      colSpan: 'full',
      render: () => (
        <Text size='sm' c='dimmed'>
          {t(`${page}.form.usageDescription`, {})}
        </Text>
      ),
    },
    {
      type: 'switch',
      name: 'gatewayEnabled',
      label: t(`${page}.form.gatewayEnabled`, {}),
      description: t(`${page}.form.gatewayEnabledDescription`, {}),
      colSpan: 'full',
    },
    number(
      'plusPercent',
      t(`${page}.form.plusPercent`, { name: economy.plusName }),
      t(`${page}.form.plusPercentDescription`, {}),
      100,
      1000,
    ),
    number(
      'plusplusPercent',
      t(`${page}.form.plusplusPercent`, { name: economy.plusplusName }),
      t(`${page}.form.plusplusPercentDescription`, { plus: economy.plusName }),
      100,
      1000,
    ),
    number(
      'discordUnlinkedPercent',
      t(`${page}.form.discordUnlinkedPercent`, {}),
      t(`${page}.form.discordUnlinkedPercentDescription`, {}),
      0,
      100,
      'full',
    ),
    number('shortHours', t(`${page}.form.shortHours`, {}), undefined, 1, 168),
    number('shortTokens', t(`${page}.form.shortTokens`, {}), t(`${page}.form.tokensDescription`, {}), 0),
    number('longDays', t(`${page}.form.longDays`, {}), undefined, 1, 90),
    number('longTokens', t(`${page}.form.longTokens`, {}), t(`${page}.form.tokensDescription`, {}), 0),
    number(
      'defaultCooldown',
      t(`${page}.form.defaultCooldown`, {}),
      t(`${page}.form.defaultCooldownDescription`, {}),
      1,
      3600,
    ),
    number('maxBusyWait', t(`${page}.form.maxBusyWait`, {}), t(`${page}.form.maxBusyWaitDescription`, {}), 1, 600),
    number('deadline', t(`${page}.form.deadline`, {}), t(`${page}.form.deadlineDescription`, {}), 10, 300),
    number('maxConcurrent', t(`${page}.form.maxConcurrent`, {}), t(`${page}.form.maxConcurrentDescription`, {}), 1, 64),
    {
      type: 'switch',
      name: 'toolsProbe',
      label: t(`${page}.form.toolsProbe`, {}),
      description: t(`${page}.form.toolsProbeDescription`, {}),
      colSpan: 'full',
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
