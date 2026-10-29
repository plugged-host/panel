// plugged.host: Admin › Settings › Accounts › rux, the moderation bot (docs/features/rux.md). Its switch
// and token are the Discord tab's moderation bot; while noprofit.space keeps rux's settings, this page
// shows them read-only and only saves what noprofit.space does not keep (the name, the secret).
import { useForm } from '@mantine/form';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import getRuxStatus from '@/api/admin/rux/getRuxStatus.ts';
import updateRuxSettings from '@/api/admin/settings/updateRuxSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import { type FieldDef, FormEngine, type TextFieldDef, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Anchor from '@/elements/typography/Anchor.tsx';
import Text from '@/elements/typography/Text.tsx';
import { ruxStatusSchema } from '@/lib/schemas/admin/rux.ts';
import { adminSettingsRuxSchema, adminSettingsRuxStoreSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';
import RuxServersField from './RuxServersField.tsx';

type RuxValues = z.infer<typeof adminSettingsRuxSchema>;
type RuxStored = z.infer<typeof adminSettingsRuxStoreSchema>;
type RuxStatus = z.infer<typeof ruxStatusSchema>;
type NumberField = {
  [K in keyof RuxValues]: RuxValues[K] extends number ? K : never;
}[keyof RuxValues];

const page = 'pages.admin.settings.tabs.rux.page';

const ruxEmptyFormValues: RuxValues = {
  name: 'rux',
  guildId: '',
  giveawayChannelId: '',
  modLogChannelId: '',
  trustedDomains: [],
  servers: [],
  timezone: 'UTC',
  locale: 'en',
  giveawayMaxCredits: 10_000,
  giveawayMaxWinners: 25,
  giveawayMaxDays: 60,
  giveawayMaxTokens: 50,
  giveawayMaxMembershipDays: 30,
  tokensCommand: true,
  announceChannelId: '',
  tokenBonusPosts: true,
  creditGranters: [],
  creditMaxPerGrant: 10_000,
  warnTimeoutAt: 3,
  warnTimeout: '24h',
  warnBanAt: 5,
  warnBan: '24h',
  ruleCooldownSeconds: 300,
  watchGuidelines: true,
  spamEnabled: true,
  spamFloodMessages: 7,
  spamFloodSeconds: 10,
  spamDuplicates: 3,
  spamDuplicateSeconds: 120,
  spamMentions: 6,
  spamEmoji: 25,
  messagesPerMinute: 6,
  maxSteps: 8,
  historyMinutes: 30,
  noprofitSecret: '',
};

// the secret never comes back: the field stays empty and keeps it
const toFormValues = (stored: RuxStored): RuxValues => ({
  ...stored,
  locale: stored.locale === 'de' ? 'de' : 'en',
  noprofitSecret: '',
});

// whether rux runs, and why not; the invite links once there is a token
function RuxState({ status }: { status: RuxStatus | null }) {
  const { t } = useTranslations();

  if (!status) {
    return null;
  }

  const invites =
    status.invites.length > 0 ? (
      <Stack gap={2} mt='xs'>
        <Text size='sm'>{t(`${page}.status.invites`, {})}</Text>
        {status.invites.map((invite) => (
          <Text key={invite.guildId} size='sm'>
            {invite.name} ({invite.guildId}):{' '}
            <Anchor href={invite.url} target='_blank' rel='noopener noreferrer' size='sm'>
              {t(`${page}.status.invite`, {})}
            </Anchor>
          </Text>
        ))}
      </Stack>
    ) : null;

  if (status.blockers.length > 0) {
    return (
      <Alert color='yellow' title={t(`${page}.status.notRunning`, {})}>
        <Stack gap={2}>
          {status.blockers.map((blocker) => (
            <Text key={blocker} size='sm'>
              {t(`${page}.status.blocker.${blocker}`, {})}
            </Text>
          ))}
        </Stack>
        {invites}
      </Alert>
    );
  }

  const bot = status.bot;
  if (bot?.phase === 'refused') {
    const reason = bot.reason === 'token' || bot.reason === 'intents' ? bot.reason : 'closed';

    return (
      <Alert color='yellow' title={t(`${page}.status.refused`, {})}>
        <Text size='sm'>
          {t(`${page}.status.reason.${reason}`, {})} · <FormattedTimestamp timestamp={bot.since} />
        </Text>
      </Alert>
    );
  }
  // Discord refused the privileged intent and the gateway connected without it
  if (bot?.reason === 'members_intent') {
    return (
      <Alert color='yellow' title={t(`${page}.status.noContent`, {})}>
        <Text size='sm'>{t(`${page}.status.noContentDescription`, {})}</Text>
        {invites}
      </Alert>
    );
  }
  if (bot?.phase !== 'ready') {
    return (
      <Alert title={t(`${page}.status.starting`, {})}>
        <Text size='sm'>{t(`${page}.status.startingDescription`, {})}</Text>
        {invites}
      </Alert>
    );
  }

  return (
    <Alert color='green' title={t(`${page}.status.online`, {})}>
      <Text size='sm'>
        {t(`${page}.status.onlineDescription`, { name: bot.username ?? status.settings.name })}{' '}
        <FormattedTimestamp timestamp={bot.since} />
      </Text>
      {invites}
    </Alert>
  );
}

export default function RuxContainer() {
  const { t } = useTranslations();
  const rux = useAdminStore((state) => state.rux);
  const appName = useGlobalStore((state) => state.settings.app.name);
  const [status, setStatus] = useState<RuxStatus | null>(null);
  const [storedSecret, setStoredSecret] = useState(!!rux.noprofitSecret);

  const loadStatus = () =>
    getRuxStatus()
      .then(setStatus)
      .catch(() => setStatus(null));

  useEffect(() => {
    loadStatus();
  }, []);

  const managed = status?.managed ?? null;

  const form = useFormEngine<RuxValues>('admin.settings.rux', {
    schema: adminSettingsRuxSchema,
    initialValues: ruxEmptyFormValues,
  });

  useHydrateForm(form, rux, toFormValues);
  useFormDraft(form, 'rux');

  // what rux runs on while noprofit.space keeps its settings: shown, never saved
  const shown = useForm<RuxValues>({ initialValues: ruxEmptyFormValues });
  useEffect(() => {
    if (status?.managed) {
      shown.setValues(toFormValues(status.settings));
    }
  }, [status]);

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsRuxSchema,
    storeKey: 'rux',
    update: (values) =>
      updateRuxSettings(values, managed !== null).then(() => {
        // a typed secret is stored from now on
        setStoredSecret((stored) => stored || !!values.noprofitSecret.trim());
        loadStatus();
      }),
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const number = (name: NumberField, min: number, max: number, description?: string): FieldDef<RuxValues> => ({
    type: 'number',
    name,
    label: t(`${page}.form.${name}`, {}),
    description,
    required: true,
    props: { min, max },
  });

  const text = (name: keyof RuxValues & string, description?: string): TextFieldDef<RuxValues> => ({
    type: 'text',
    name,
    label: t(`${page}.form.${name}`, {}),
    description,
  });

  const intro = (name: string, description: string): FieldDef<RuxValues> => ({
    type: 'custom',
    name,
    colSpan: 'full',
    render: () => (
      <Text size='sm' c='dimmed'>
        {description}
      </Text>
    ),
  });

  const chain = (models: string[]) => (models.length > 0 ? models.join(' → ') : t(`${page}.form.aiNone`, {}));

  // the bot's name: saved even while noprofit.space keeps the rest
  const head: FieldDef<RuxValues>[] = [
    { type: 'divider', name: 'botDivider', label: t(`${page}.form.bot`, {}) },
    intro(
      'botIntro',
      t(`${page}.form.botDescription`, {
        admins: status?.noprofitConnected ? t(`${page}.form.adminsNoprofit`, {}) : t(`${page}.form.adminsPanel`, {}),
      }),
    ),
    { ...text('name', t(`${page}.form.nameDescription`, {})), required: true },
  ];

  const body: FieldDef<RuxValues>[] = [
    text('guildId', t(`${page}.form.guildIdDescription`, {})),
    {
      type: 'timezone',
      name: 'timezone',
      label: t(`${page}.form.timezone`, {}),
      description: t(`${page}.form.timezoneDescription`, {}),
      required: true,
    },
    {
      type: 'select',
      name: 'locale',
      label: t(`${page}.form.locale`, {}),
      description: t(`${page}.form.localeDescription`, {}),
      options: [
        { value: 'en', label: 'English' },
        { value: 'de', label: 'Deutsch' },
      ],
    },
    number('messagesPerMinute', 1, 120, t(`${page}.form.messagesPerMinuteDescription`, {})),
    number('maxSteps', 2, 30, t(`${page}.form.maxStepsDescription`, {})),
    number('historyMinutes', 1, 1440, t(`${page}.form.historyMinutesDescription`, {})),

    { type: 'divider', name: 'giveawaysDivider', label: t(`${page}.form.giveaways`, {}) },
    text('giveawayChannelId', t(`${page}.form.giveawayChannelIdDescription`, {})),
    text('modLogChannelId', t(`${page}.form.modLogChannelIdDescription`, {})),
    number('giveawayMaxCredits', 1, 1_000_000),
    number('giveawayMaxWinners', 1, 500),
    number('giveawayMaxDays', 1, 365, t(`${page}.form.giveawayMaxDaysDescription`, {})),
    {
      type: 'tags',
      name: 'creditGranters',
      label: t(`${page}.form.creditGranters`, {}),
      description: t(`${page}.form.creditGrantersDescription`, {}),
    },
    number('creditMaxPerGrant', 1, 1_000_000),

    { type: 'divider', name: 'serversDivider', label: t(`${page}.form.servers`, {}) },
    {
      type: 'custom',
      name: 'servers',
      colSpan: 'full',
      render: (target) => <RuxServersField form={target} appName={appName} readOnly={managed !== null} />,
    },

    { type: 'divider', name: 'warningsDivider', label: t(`${page}.form.warnings`, {}) },
    intro('warningsIntro', t(`${page}.form.warningsDescription`, {})),
    number('warnTimeoutAt', 0, 100),
    text('warnTimeout', t(`${page}.form.warnTimeoutDescription`, {})),
    number('warnBanAt', 0, 100),
    text('warnBan', t(`${page}.form.warnBanDescription`, {})),

    { type: 'divider', name: 'aiDivider', label: t(`${page}.form.ai`, {}) },
    intro(
      'aiIntro',
      `${t(`${page}.form.aiDescription`, {
        orders: chain(status?.orderModels ?? []),
        checks: chain(status?.checkModels ?? []),
      })}${status?.aiSource === 'noprofit' ? ` ${t(`${page}.form.aiNoprofit`, {})}` : ''}`,
    ),
    {
      type: 'switch',
      name: 'watchGuidelines',
      label: t(`${page}.form.watchGuidelines`, {}),
      description: t(`${page}.form.watchGuidelinesDescription`, {}),
    },
    {
      type: 'switch',
      name: 'spamEnabled',
      label: t(`${page}.form.spamEnabled`, {}),
      description: t(`${page}.form.spamEnabledDescription`, {}),
    },
    {
      ...number('spamFloodMessages', 2, 100, t(`${page}.form.spamFloodMessagesDescription`, {})),
      when: (values) => values.spamEnabled,
    },
    { ...number('spamFloodSeconds', 1, 600), when: (values) => values.spamEnabled },
    {
      ...number('spamDuplicates', 2, 100, t(`${page}.form.spamDuplicatesDescription`, {})),
      when: (values) => values.spamEnabled,
    },
    { ...number('spamDuplicateSeconds', 1, 3600), when: (values) => values.spamEnabled },
    { ...number('spamMentions', 2, 100), when: (values) => values.spamEnabled },
    { ...number('spamEmoji', 2, 500), when: (values) => values.spamEnabled },
    number('ruleCooldownSeconds', 0, 86400, t(`${page}.form.ruleCooldownSecondsDescription`, {})),
    {
      type: 'tags',
      name: 'trustedDomains',
      label: t(`${page}.form.trustedDomains`, {}),
      description: t(`${page}.form.trustedDomainsDescription`, {}),
      placeholder: 'example.com',
    },
  ];

  // plugged.host: tokens and memberships are always the panel's, so these stay editable while
  // noprofit.space keeps the rest
  const tokens: FieldDef<RuxValues>[] = [
    { type: 'divider', name: 'tokensDivider', label: t(`${page}.form.tokens`, {}) },
    {
      type: 'switch',
      name: 'tokensCommand',
      label: t(`${page}.form.tokensCommand`, {}),
      description: t(`${page}.form.tokensCommandDescription`, {}),
    },
    number('giveawayMaxTokens', 1, 100_000, t(`${page}.form.giveawayMaxTokensDescription`, {})),
    number('giveawayMaxMembershipDays', 1, 365, t(`${page}.form.giveawayMaxMembershipDaysDescription`, {})),
    text('announceChannelId', t(`${page}.form.announceChannelIdDescription`, {})),
    {
      type: 'switch',
      name: 'tokenBonusPosts',
      label: t(`${page}.form.tokenBonusPosts`, {}),
      description: t(`${page}.form.tokenBonusPostsDescription`, {}),
    },
  ];

  // noprofit.space's rux routes: only while it is connected
  const tail: FieldDef<RuxValues>[] = status?.noprofitConnected
    ? [
        { type: 'divider', name: 'noprofitDivider', label: t(`${page}.form.noprofit`, {}) },
        {
          type: 'password',
          name: 'noprofitSecret',
          label: t(`${page}.form.noprofitSecret`, {}),
          description: `${t(`${page}.form.noprofitSecretDescription`, {})} ${
            storedSecret ? t(`${page}.form.secretStored`, {}) : t(`${page}.form.secretMissing`, {})
          }`,
          props: { autoComplete: 'new-password' },
        },
      ]
    : [];

  return (
    <AdminSubContentContainer title={t(`${page}.title`, {})} titleOrder={2}>
      <Stack gap='md'>
        <RuxState status={status} />

        {managed && (
          <Alert title={t(`${page}.managed.title`, { by: managed.by })}>
            <Text size='sm'>
              {t(`${page}.managed.description`, { by: managed.by })}{' '}
              {managed.fetchedAt > 0 && (
                <>
                  {t(`${page}.managed.fetched`, {})} <FormattedTimestamp timestamp={managed.fetchedAt * 1000} />.
                </>
              )}
            </Text>
            {managed.url && (
              <Anchor href={managed.url} target='_blank' rel='noopener noreferrer' size='sm'>
                {t(`${page}.managed.open`, { by: managed.by })}
              </Anchor>
            )}
          </Alert>
        )}

        <form onSubmit={form.onSubmit(submit)}>
          {managed ? (
            <Stack gap='md'>
              <FormEngine form={form} fields={head} />
              <fieldset disabled className='contents'>
                <FormEngine form={shown} fields={body} />
              </fieldset>
              <FormEngine form={form} fields={[...tokens, ...tail]} />
            </Stack>
          ) : (
            <FormEngine form={form} fields={[...head, ...body, ...tokens, ...tail]} />
          )}

          <Group mt='md'>
            <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
          </Group>
        </form>
      </Stack>
    </AdminSubContentContainer>
  );
}
