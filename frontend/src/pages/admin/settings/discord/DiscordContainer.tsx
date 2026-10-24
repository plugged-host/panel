// plugged.host: Admin › Settings › Accounts › Discord (docs/features/discord.md)
import { useEffect, useState } from 'react';
import { z } from 'zod';
import getDiscordStatus from '@/api/admin/discord/getDiscordStatus.ts';
import getOAuthProviders from '@/api/admin/oauth-providers/getOAuthProviders.ts';
import updateDiscordSettings from '@/api/admin/settings/updateDiscordSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { discordBotStatusSchema } from '@/lib/schemas/admin/discord.ts';
import { adminSettingsDiscordSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type DiscordValues = z.infer<typeof adminSettingsDiscordSchema>;
type TokenField = 'botToken' | 'assistantBotToken' | 'moderationBotToken';
type NumberField =
  | 'tierCpu'
  | 'tierMemory'
  | 'tierDisk'
  | 'creditsPerInvite'
  | 'minAccountAgeDays'
  | 'inviteMaturityHours'
  | 'boostOneCredits'
  | 'boostTwoCredits';
type BotStatus = z.infer<typeof discordBotStatusSchema>;

const page = 'pages.admin.settings.tabs.discord.page';

const discordEmptyFormValues: DiscordValues = {
  enabled: false,
  guildId: '',
  botToken: '',
  inviteUrl: '',
  oauthProviderUuid: null,
  tierCpu: 0,
  tierMemory: 0,
  tierDisk: 0,
  creditsPerInvite: 50,
  minAccountAgeDays: 30,
  inviteMaturityHours: 6,
  inviteClawback: true,
  boostOneCredits: 500,
  boostTwoCredits: 1250,
  boostTwoRoleId: '',
  subdomainsRequireLink: false,
  subdomainsRequireMember: false,
  rewardsBotEnabled: false,
  assistantBotEnabled: false,
  assistantBotToken: '',
  moderationBotEnabled: false,
  moderationBotToken: '',
};

// what the primary instance said about the bots that are on
function BotsState({ bots }: { bots: BotStatus[] | null }) {
  const { t } = useTranslations();

  if (!bots || bots.length === 0) {
    return (
      <Text size='sm' c='dimmed'>
        {t(`${page}.status.none`, {})}
      </Text>
    );
  }

  return (
    <Stack gap={4}>
      {bots.map((bot) => (
        <Text key={bot.bot} size='sm'>
          {t(`${page}.status.bot.${bot.bot}`, {})}:{' '}
          {bot.phase === 'ready'
            ? t(`${page}.status.ready`, { name: bot.username ?? '' })
            : t(`${page}.status.${bot.phase}`, {})}
          {bot.reason && (
            <Text component='span' size='sm' c='dimmed'>
              {' '}
              ({t(`${page}.status.reason.${bot.reason}`, {})})
            </Text>
          )}
          <Text component='span' size='sm' c='dimmed'>
            {' '}
            · <FormattedTimestamp timestamp={bot.since} />
          </Text>
        </Text>
      ))}
    </Stack>
  );
}

export default function DiscordContainer() {
  const { t } = useTranslations();
  const discord = useAdminStore((state) => state.discord);
  // stored tokens come back as placeholders
  const [stored, setStored] = useState<Record<TokenField, boolean>>(() => ({
    botToken: !!discord.botToken,
    assistantBotToken: !!discord.assistantBotToken,
    moderationBotToken: !!discord.moderationBotToken,
  }));
  const [providers, setProviders] = useState<{ value: string; label: string }[]>([]);
  const [bots, setBots] = useState<BotStatus[] | null>(null);

  useEffect(() => {
    getOAuthProviders(1, undefined, 100)
      .then((page) => setProviders(page.data.map((provider) => ({ value: provider.uuid, label: provider.name }))))
      .catch(() => setProviders([]));
    getDiscordStatus()
      .then(setBots)
      .catch(() => setBots(null));
  }, []);

  const form = useFormEngine<DiscordValues>('admin.settings.discord', {
    schema: adminSettingsDiscordSchema,
    initialValues: discordEmptyFormValues,
  });

  // the tokens never come back: the fields stay empty and keep them
  useHydrateForm(form, discord, (discord) => ({
    ...discord,
    botToken: '',
    assistantBotToken: '',
    moderationBotToken: '',
  }));
  useFormDraft(form, 'discord');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsDiscordSchema,
    storeKey: 'discord',
    // a typed token is stored from now on; the account menu follows the switch at once
    update: (values) =>
      updateDiscordSettings(values).then(() => {
        setStored((stored) => ({
          botToken: stored.botToken || !!values.botToken.trim(),
          assistantBotToken: stored.assistantBotToken || !!values.assistantBotToken.trim(),
          moderationBotToken: stored.moderationBotToken || !!values.moderationBotToken.trim(),
        }));
        useGlobalStore.getState().updateSettings({
          discord: { enabled: values.enabled && values.guildId !== '', inviteUrl: values.inviteUrl },
        });
      }),
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const number = (name: NumberField, max: number, description?: string): FieldDef<DiscordValues> => ({
    type: 'number',
    name,
    label: t(`${page}.form.${name}`, {}),
    description,
    required: true,
    props: { min: 0, max },
  });

  const token = (
    name: TokenField,
    hint: string | null,
    when?: (values: DiscordValues) => boolean,
  ): FieldDef<DiscordValues> => {
    const state = stored[name] ? t(`${page}.form.tokenStored`, {}) : t(`${page}.form.tokenMissing`, {});

    return {
      type: 'password',
      name,
      label: t(`${page}.form.${name}`, {}),
      description: hint ? `${hint} ${state}` : state,
      props: { autoComplete: 'new-password' },
      when,
    };
  };

  const fields: FieldDef<DiscordValues>[] = [
    {
      type: 'switch',
      name: 'enabled',
      label: t(`${page}.form.enabled`, {}),
      description: t(`${page}.form.enabledDescription`, {}),
    },
    {
      type: 'text',
      name: 'guildId',
      label: t(`${page}.form.guildId`, {}),
      description: t(`${page}.form.guildIdDescription`, {}),
    },
    token('botToken', t(`${page}.form.botTokenDescription`, {})),
    {
      type: 'text',
      name: 'inviteUrl',
      label: t(`${page}.form.inviteUrl`, {}),
      description: t(`${page}.form.inviteUrlDescription`, {}),
    },
    {
      type: 'select',
      name: 'oauthProviderUuid',
      label: t(`${page}.form.oauthProvider`, {}),
      description: t(`${page}.form.oauthProviderDescription`, {}),
      options: providers,
      props: { clearable: true, searchable: true, placeholder: t(`${page}.form.oauthProviderPlaceholder`, {}) },
    },
    {
      type: 'divider',
      name: 'tierDivider',
      label: t(`${page}.form.tier`, {}),
    },
    number('tierCpu', 3200, t(`${page}.form.tierDescription`, {})),
    number('tierMemory', 262144),
    number('tierDisk', 1048576),
    {
      type: 'divider',
      name: 'invitesDivider',
      label: t(`${page}.form.invites`, {}),
    },
    number('creditsPerInvite', 1_000_000, t(`${page}.form.creditsPerInviteDescription`, {})),
    number('minAccountAgeDays', 3650, t(`${page}.form.minAccountAgeDaysDescription`, {})),
    number('inviteMaturityHours', 720, t(`${page}.form.inviteMaturityHoursDescription`, {})),
    {
      type: 'switch',
      name: 'inviteClawback',
      label: t(`${page}.form.inviteClawback`, {}),
      description: t(`${page}.form.inviteClawbackDescription`, {}),
    },
    {
      type: 'divider',
      name: 'boostsDivider',
      label: t(`${page}.form.boosts`, {}),
    },
    number('boostOneCredits', 10_000_000),
    number('boostTwoCredits', 10_000_000, t(`${page}.form.boostTwoCreditsDescription`, {})),
    {
      type: 'text',
      name: 'boostTwoRoleId',
      label: t(`${page}.form.boostTwoRoleId`, {}),
      description: t(`${page}.form.boostTwoRoleIdDescription`, {}),
    },
    {
      type: 'divider',
      name: 'subdomainsDivider',
      label: t(`${page}.form.subdomains`, {}),
    },
    {
      type: 'switch',
      name: 'subdomainsRequireLink',
      label: t(`${page}.form.subdomainsRequireLink`, {}),
      description: t(`${page}.form.subdomainsRequireLinkDescription`, {}),
    },
    {
      type: 'switch',
      name: 'subdomainsRequireMember',
      label: t(`${page}.form.subdomainsRequireMember`, {}),
      description: t(`${page}.form.subdomainsRequireMemberDescription`, {}),
    },
    {
      type: 'divider',
      name: 'botsDivider',
      label: t(`${page}.form.bots`, {}),
    },
    {
      type: 'switch',
      name: 'rewardsBotEnabled',
      label: t(`${page}.form.rewardsBotEnabled`, {}),
      description: t(`${page}.form.rewardsBotEnabledDescription`, {}),
    },
    {
      type: 'switch',
      name: 'assistantBotEnabled',
      label: t(`${page}.form.assistantBotEnabled`, {}),
      description: t(`${page}.form.assistantBotEnabledDescription`, {}),
    },
    token('assistantBotToken', null, (values) => values.assistantBotEnabled),
    {
      type: 'switch',
      name: 'moderationBotEnabled',
      label: t(`${page}.form.moderationBotEnabled`, {}),
      description: t(`${page}.form.moderationBotEnabledDescription`, {}),
    },
    token('moderationBotToken', null, (values) => values.moderationBotEnabled),
  ];

  return (
    <AdminSubContentContainer title={t(`${page}.title`, {})} titleOrder={2}>
      <Stack gap='xl'>
        <form onSubmit={form.onSubmit(submit)}>
          <FormEngine form={form} fields={fields} />

          <Group mt='md'>
            <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
          </Group>
        </form>

        <Stack gap='xs'>
          <Text fw={600}>{t(`${page}.status.title`, {})}</Text>
          <BotsState bots={bots} />
        </Stack>
      </Stack>
    </AdminSubContentContainer>
  );
}
