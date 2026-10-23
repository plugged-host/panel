// plugged.host: Admin › Settings › Economy, one tab per part (docs/features/economy.md)
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useState } from 'react';
import { z } from 'zod';
import updateEconomySettings from '@/api/admin/settings/updateEconomySettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminSettingsEconomySchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type EconomyValues = z.infer<typeof adminSettingsEconomySchema>;

export type EconomyPart = 'store' | 'tokens' | 'memberships';

// What each tab edits. A tab saves the whole section: its own fields over what is stored, so an
// unsaved edit in another tab is neither sent nor lost.
const partFields: Record<EconomyPart, (keyof EconomyValues)[]> = {
  store: [
    'creditsEnabled',
    'startingCredits',
    'cpuStep',
    'cpuPrice',
    'limitCpu',
    'memoryStep',
    'memoryPrice',
    'limitMemory',
    'diskStep',
    'diskPrice',
    'limitDisk',
    'priceFloor',
    'softLimit',
    'softMultiplier',
    'doubling',
  ],
  tokens: [
    'tokensEnabled',
    'dailyTokens',
    'claimMinutes',
    'linkvertiseEnabled',
    'linkvertiseToken',
    'linkvertisePublisher',
    'linkvertiseWhitelist',
    'linkvertiseLink',
    'linkvertiseReward',
    'linkvertiseDailyCap',
    'vyloEnabled',
    'vyloApiKey',
    'vyloReward',
    'vyloDailyCap',
    'vyloMinSeconds',
  ],
  memberships: [
    'membershipEnabled',
    'plusName',
    'plusplusName',
    'plusTokens',
    'plusplusTokens',
    'plusplusTokenDays',
    'plusplusCredits',
    'plusplusCreditDays',
    'plusplusCpu',
    'plusBurstCores',
    'plusplusBurstCores',
  ],
};

const titles = {
  store: 'pages.admin.settings.tabs.economy.page.storeTitle',
  tokens: 'pages.admin.settings.tabs.economy.page.tokensTitle',
  memberships: 'pages.admin.settings.tabs.economy.page.membershipsTitle',
} as const;

export default function EconomyContainer({ part }: { part: EconomyPart }) {
  const { t } = useTranslations();
  const economy = useAdminStore((state) => state.economy);
  const page = 'pages.admin.settings.tabs.economy.page.form';
  const [stored, setStored] = useState(() => ({
    linkvertiseToken: economy.linkvertiseToken !== null,
    vyloApiKey: economy.vyloApiKey !== null,
  }));

  const form = useForm<EconomyValues>({
    initialValues: { ...economy, linkvertiseToken: '', vyloApiKey: '' },
    ...liveValidation,
    validate: zod4Resolver(adminSettingsEconomySchema),
  });

  // the secrets are never sent to the browser: an empty field keeps the stored one
  useHydrateForm(form, economy, (values: EconomyValues) => ({ ...values, linkvertiseToken: '', vyloApiKey: '' }));
  useFormDraft(form, `economy.${part}`);

  // The stored secrets come back as "" (set) or null (not set): sent again, a set one is kept and an
  // unset one stays unset. Only the tokens tab sends what was typed.
  const schema = adminSettingsEconomySchema.transform(
    (values): EconomyValues => ({
      ...economy,
      ...(Object.fromEntries(partFields[part].map((field) => [field, values[field]])) as Partial<EconomyValues>),
    }),
  );

  const { loading, submit } = useSettingsSection({
    form,
    schema,
    storeKey: 'economy',
    update: (values) =>
      updateEconomySettings(values).then(() =>
        setStored((stored) => ({
          linkvertiseToken: stored.linkvertiseToken || !!values.linkvertiseToken,
          vyloApiKey: stored.vyloApiKey || !!values.vyloApiKey,
        })),
      ),
    successMessage: t('pages.admin.settings.tabs.economy.page.toast.updated', {}),
  });

  const secret = (isStored: boolean) => (isStored ? t(`${page}.secretStored`, {}) : t(`${page}.secretMissing`, {}));

  const number = (
    name: keyof EconomyValues & string,
    label: string,
    description: string | null,
    min: number,
    max: number,
    when?: (values: EconomyValues) => boolean,
  ): FieldDef<EconomyValues> => ({
    type: 'number',
    name,
    label,
    ...(description ? { description } : {}),
    required: true,
    props: { min, max },
    ...(when ? { when } : {}),
  });

  const credits = (values: EconomyValues) => values.creditsEnabled;
  const tokens = (values: EconomyValues) => values.tokensEnabled;
  const membership = (values: EconomyValues) => values.membershipEnabled;

  const fields: Record<EconomyPart, FieldDef<EconomyValues>[]> = {
    store: [
      {
        type: 'switch',
        name: 'creditsEnabled',
        label: t(`${page}.creditsEnabled`, {}),
        description: t(`${page}.creditsEnabledDescription`, {}),
      },
      number(
        'startingCredits',
        t(`${page}.startingCredits`, {}),
        t(`${page}.startingCreditsDescription`, {}),
        0,
        1_000_000,
        credits,
      ),
      { type: 'divider', name: 'storeDivider', label: t(`${page}.storeSection`, {}), when: credits },
      {
        type: 'custom',
        name: 'storeNote',
        when: credits,
        render: () => (
          <Text size='sm' c='dimmed'>
            {t(`${page}.storeNote`, {})}
          </Text>
        ),
      },
      number('cpuStep', t(`${page}.cpuStep`, {}), t(`${page}.stepDescription`, {}), 0, 10_000, credits),
      number('cpuPrice', t(`${page}.cpuPrice`, {}), null, 0, 1_000_000, credits),
      number('limitCpu', t(`${page}.limitCpu`, {}), null, 1, 100_000, credits),
      number('memoryStep', t(`${page}.memoryStep`, {}), null, 0, 1_048_576, credits),
      number('memoryPrice', t(`${page}.memoryPrice`, {}), null, 0, 1_000_000, credits),
      number('limitMemory', t(`${page}.limitMemory`, {}), null, 1, 10_485_760, credits),
      number('diskStep', t(`${page}.diskStep`, {}), null, 0, 10_485_760, credits),
      number('diskPrice', t(`${page}.diskPrice`, {}), null, 0, 1_000_000, credits),
      number('limitDisk', t(`${page}.limitDisk`, {}), null, 1, 104_857_600, credits),
      number('priceFloor', t(`${page}.priceFloor`, {}), t(`${page}.priceFloorDescription`, {}), 0, 100, credits),
      number('softLimit', t(`${page}.softLimit`, {}), t(`${page}.softLimitDescription`, {}), 100, 1000, credits),
      number(
        'softMultiplier',
        t(`${page}.softMultiplier`, {}),
        t(`${page}.softMultiplierDescription`, {}),
        1,
        100,
        credits,
      ),
      number('doubling', t(`${page}.doubling`, {}), t(`${page}.doublingDescription`, {}), 1, 100, credits),
    ],
    tokens: [
      {
        type: 'switch',
        name: 'tokensEnabled',
        label: t(`${page}.tokensEnabled`, {}),
        description: t(`${page}.tokensEnabledDescription`, {}),
      },
      number('dailyTokens', t(`${page}.dailyTokens`, {}), t(`${page}.dailyTokensDescription`, {}), 0, 100, tokens),
      number('claimMinutes', t(`${page}.claimMinutes`, {}), t(`${page}.claimMinutesDescription`, {}), 1, 1440, tokens),

      { type: 'divider', name: 'earnDivider', label: t(`${page}.earnSection`, {}), when: tokens },
      {
        type: 'switch',
        name: 'linkvertiseEnabled',
        label: t(`${page}.linkvertiseEnabled`, {}),
        description: t(`${page}.linkvertiseEnabledDescription`, {}),
        when: tokens,
      },
      {
        type: 'password',
        name: 'linkvertiseToken',
        label: t(`${page}.linkvertiseToken`, {}),
        description: secret(stored.linkvertiseToken),
        props: { autoComplete: 'new-password' },
        when: (values) => values.tokensEnabled && values.linkvertiseEnabled,
      },
      {
        type: 'text',
        name: 'linkvertisePublisher',
        label: t(`${page}.linkvertisePublisher`, {}),
        description: t(`${page}.linkvertisePublisherDescription`, {}),
        when: (values) => values.tokensEnabled && values.linkvertiseEnabled,
      },
      {
        type: 'text',
        name: 'linkvertiseWhitelist',
        label: t(`${page}.linkvertiseWhitelist`, {}),
        description: t(`${page}.linkvertiseWhitelistDescription`, {}),
        when: (values) => values.tokensEnabled && values.linkvertiseEnabled && !!values.linkvertisePublisher,
      },
      {
        type: 'text',
        name: 'linkvertiseLink',
        label: t(`${page}.linkvertiseLink`, {}),
        description: t(`${page}.linkvertiseLinkDescription`, { returnPlain: '{return}', returnB64: '{return_b64}' }),
        when: (values) => values.tokensEnabled && values.linkvertiseEnabled && !values.linkvertisePublisher,
      },
      number(
        'linkvertiseReward',
        t(`${page}.reward`, {}),
        null,
        1,
        1000,
        (values) => values.tokensEnabled && values.linkvertiseEnabled,
      ),
      number(
        'linkvertiseDailyCap',
        t(`${page}.dailyCap`, {}),
        t(`${page}.dailyCapDescription`, {}),
        0,
        1000,
        (values) => values.tokensEnabled && values.linkvertiseEnabled,
      ),
      {
        type: 'switch',
        name: 'vyloEnabled',
        label: t(`${page}.vyloEnabled`, {}),
        description: t(`${page}.vyloEnabledDescription`, {}),
        when: tokens,
      },
      {
        type: 'password',
        name: 'vyloApiKey',
        label: t(`${page}.vyloApiKey`, {}),
        description: secret(stored.vyloApiKey),
        props: { autoComplete: 'new-password' },
        when: (values) => values.tokensEnabled && values.vyloEnabled,
      },
      number(
        'vyloReward',
        t(`${page}.reward`, {}),
        null,
        1,
        1000,
        (values) => values.tokensEnabled && values.vyloEnabled,
      ),
      number(
        'vyloDailyCap',
        t(`${page}.dailyCap`, {}),
        t(`${page}.dailyCapDescription`, {}),
        0,
        1000,
        (values) => values.tokensEnabled && values.vyloEnabled,
      ),
      number(
        'vyloMinSeconds',
        t(`${page}.vyloMinSeconds`, {}),
        t(`${page}.vyloMinSecondsDescription`, {}),
        0,
        600,
        (values) => values.tokensEnabled && values.vyloEnabled,
      ),
    ],
    memberships: [
      {
        type: 'switch',
        name: 'membershipEnabled',
        label: t(`${page}.membershipEnabled`, {}),
        description: t(`${page}.membershipEnabledDescription`, {}),
      },
      {
        type: 'text',
        name: 'plusName',
        label: t(`${page}.plusName`, {}),
        description: t(`${page}.plusNameDescription`, {}),
        required: true,
        when: membership,
      },
      {
        type: 'text',
        name: 'plusplusName',
        label: t(`${page}.plusplusName`, {}),
        description: t(`${page}.plusplusNameDescription`, {}),
        required: true,
        when: membership,
      },
      number('plusTokens', t(`${page}.plusTokens`, {}), null, 0, 100_000, membership),
      number('plusplusTokens', t(`${page}.plusplusTokens`, {}), null, 0, 100_000, membership),
      number('plusplusTokenDays', t(`${page}.plusplusTokenDays`, {}), null, 1, 365, membership),
      number(
        'plusplusCredits',
        t(`${page}.plusplusCredits`, {}),
        t(`${page}.plusplusCreditsDescription`, {}),
        0,
        1_000_000,
        membership,
      ),
      number('plusplusCreditDays', t(`${page}.plusplusCreditDays`, {}), null, 1, 365, membership),
      number('plusplusCpu', t(`${page}.plusplusCpu`, {}), t(`${page}.plusplusCpuDescription`, {}), 0, 1000, membership),
      number(
        'plusBurstCores',
        t(`${page}.plusBurstCores`, {}),
        t(`${page}.plusBurstCoresDescription`, {}),
        0,
        16,
        membership,
      ),
      number(
        'plusplusBurstCores',
        t(`${page}.plusplusBurstCores`, {}),
        t(`${page}.plusplusBurstCoresDescription`, {}),
        0,
        16,
        membership,
      ),
    ],
  };

  return (
    <AdminSubContentContainer title={t(titles[part], {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields[part]} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
