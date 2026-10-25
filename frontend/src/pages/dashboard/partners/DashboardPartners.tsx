// plugged.host: Account › Partners, the partner program (docs/features/partners.md): what a Discord
// community needs, the application form, the partnership with its monthly credits and payments,
// and earlier applications. Laid out like the Discord rewards page; at most one filled button:
// linking Discord while there is no link, else Apply.
import { Button as MantineButton } from '@mantine/core';
import { useForm } from '@mantine/form';
import { useQueryClient } from '@tanstack/react-query';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { ReactNode, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import applyPartner, { partnerRefusal } from '@/api/me/partners/applyPartner.ts';
import checkPartnership from '@/api/me/partners/checkPartnership.ts';
import getPartners from '@/api/me/partners/getPartners.ts';
import withdrawPartner from '@/api/me/partners/withdrawPartner.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Select from '@/elements/input/Select.tsx';
import ServerMultiSelect from '@/elements/input/ServerMultiSelect.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  partnerApplicationSchema,
  partnerApplySchema,
  partnerProblem,
  partnerRefusalSchema,
  partnersSchema,
} from '@/lib/schemas/user/partners.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type Partners = z.infer<typeof partnersSchema>;
type Application = z.infer<typeof partnerApplicationSchema>;
type Refusal = z.infer<typeof partnerRefusalSchema>;
type Translations = ReturnType<typeof useTranslations>;

const page = 'pages.account.partners';

const PROBLEM_KEYS = {
  not_linked: 'notLinked',
  invalid_link: 'invalidLink',
  unknown_invite: 'unknownInvite',
  bot_missing: 'botMissing',
  too_few_members: 'tooFewMembers',
  not_admin: 'notAdmin',
  channel_missing: 'channelMissing',
  channel_hidden: 'channelHidden',
  channel_link: 'channelLink',
  pending: 'pending',
  taken: 'taken',
  cooldown: 'cooldown',
} as const satisfies Record<z.infer<typeof partnerProblem>, string>;

const PARTNERSHIPS = ['approved', 'paused', 'suspended'];

function formatDate(date: Date, language: string): string {
  return date.toLocaleDateString(intlLocale(language), { year: 'numeric', month: 'long', day: 'numeric' });
}

/** A check's or refusal's problem in the reader's words. */
function problemText(
  { t, language }: Translations,
  code: string,
  partners: Partners,
  appName: string,
  extra: { members?: number | null; channel?: string | null; applyAfter?: Date | null } = {},
): string {
  if (!Object.hasOwn(PROBLEM_KEYS, code)) {
    return code;
  }
  const key = PROBLEM_KEYS[code as z.infer<typeof partnerProblem>];
  const list = new Intl.ListFormat(intlLocale(language), { style: 'long', type: 'disjunction' });

  return t(`${page}.problems.${key}`, {
    bot: partners.botName,
    members: partners.minMembers.toLocaleString(),
    count: (extra.members ?? 0).toLocaleString(),
    app: appName,
    channel: extra.channel ? `#${extra.channel}` : appName,
    links: list.format(partners.channelLinks),
    date: extra.applyAfter ? formatDate(extra.applyAfter, language) : '',
  });
}

// a full-width row: what it is and a line about it on the left, something on the right
function Row({ text, hint, children }: { text: ReactNode; hint?: ReactNode; children?: ReactNode }) {
  return (
    <div className='theme-eco-tx'>
      <div className='flex min-w-0 flex-col'>
        <span className='theme-eco-text'>{text}</span>
        {hint && <span className='theme-eco-small'>{hint}</span>}
      </div>
      {children}
    </div>
  );
}

// a link to Discord or noprofit.space, in a new tab
function ExternalButton({ href, filled, children }: { href: string; filled: boolean; children: ReactNode }) {
  return (
    <MantineButton
      component='a'
      href={href}
      target='_blank'
      rel='noopener noreferrer'
      variant={filled ? 'filled' : 'default'}
      fw={600}
    >
      {children}
    </MantineButton>
  );
}

function Requirements({ partners }: { partners: Partners }) {
  const translations = useTranslations();
  const { t, language } = translations;
  const navigate = useNavigate();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const list = new Intl.ListFormat(intlLocale(language), { style: 'long', type: 'disjunction' });
  const names = list.format(partners.channelNames.map((name) => `“${name}”`));

  return (
    <section className='flex flex-col gap-3'>
      <h3 className='theme-eco-heading'>{t(`${page}.requirements.title`, {})}</h3>

      <div className='theme-eco-list'>
        <Row
          text={t(`${page}.requirements.linked`, {})}
          hint={
            partners.discord
              ? t(`${page}.requirements.linkedAs`, { name: partners.discord.username ?? partners.discord.id })
              : t(`${page}.requirements.notLinked`, {})
          }
        >
          {partners.discord ? (
            <Badge color='green' variant='light'>
              {t(`${page}.requirements.done`, {})}
            </Badge>
          ) : partners.linkUrl ? (
            <ExternalButton href={partners.linkUrl} filled>
              {t(`${page}.requirements.linkNoprofit`, {})}
            </ExternalButton>
          ) : (
            <Button onClick={() => navigate('/account/oauth-links')}>{t(`${page}.requirements.link`, {})}</Button>
          )}
        </Row>
        <Row
          text={t(`${page}.requirements.bot`, { bot: partners.botName })}
          hint={t(`${page}.requirements.botHint`, {})}
        >
          {partners.botInviteUrl && (
            <ExternalButton href={partners.botInviteUrl} filled={false}>
              {t(`${page}.requirements.addBot`, { bot: partners.botName })}
            </ExternalButton>
          )}
        </Row>
        <Row
          text={t(`${page}.requirements.members`, { members: partners.minMembers.toLocaleString() })}
          hint={t(`${page}.requirements.membersHint`, {})}
        />
        <Row text={t(`${page}.requirements.admin`, {})} hint={t(`${page}.requirements.adminHint`, {})} />
        {partners.channelRequired && (
          <Row
            text={t(`${page}.requirements.channel`, { app: appName })}
            hint={
              partners.channelLinks.length > 0
                ? `${t(`${page}.requirements.channelHint`, { names })} ${t(`${page}.requirements.channelLinkHint`, {
                    links: list.format(partners.channelLinks),
                  })}`
                : t(`${page}.requirements.channelHint`, { names })
            }
          />
        )}
        <Row text={t(`${page}.requirements.game`, {})} hint={t(`${page}.requirements.gameHint`, {})} />
      </div>
    </section>
  );
}

function ApplyForm({ partners, onApplied }: { partners: Partners; onApplied: (partners: Partners) => void }) {
  const translations = useTranslations();
  const { t } = translations;
  const { addToast } = useToast();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const [loading, setLoading] = useState(false);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  const [servers, setServers] = useState<{ uuid: string; name: string }[]>([]);

  const form = useForm<z.infer<typeof partnerApplySchema>>({
    initialValues: {
      server: '',
      game: 'fivem',
      description: '',
      servers: [],
    },
    ...liveValidation,
    validate: zod4Resolver(partnerApplySchema),
  });

  const submit = (values: z.infer<typeof partnerApplySchema>) => {
    setLoading(true);
    setRefusal(null);
    applyPartner({ ...values, servers: servers.map((server) => server.uuid) })
      .then((after) => {
        addToast(t(`${page}.form.sent`, {}), 'success');
        form.reset();
        setServers([]);
        onApplied(after);
      })
      .catch((error) => {
        const refused = partnerRefusal(error);
        if (refused) {
          setRefusal(refused);
        } else {
          addToast(httpErrorToHuman(error), 'error');
        }
      })
      .finally(() => setLoading(false));
  };

  return (
    <section className='flex flex-col gap-3'>
      <h3 className='theme-eco-heading'>{t(`${page}.form.title`, {})}</h3>

      <form className='theme-eco-card' onSubmit={form.onSubmit(submit)}>
        <TextInput
          withAsterisk
          label={t(`${page}.form.server`, {})}
          description={t(`${page}.form.serverHint`, {})}
          {...form.getInputProps('server')}
        />
        <Select
          withAsterisk
          label={t(`${page}.form.game`, {})}
          data={(['fivem', 'minecraft', 'both'] as const).map((game) => ({
            value: game,
            label: t(`${page}.games.${game}`, {}),
          }))}
          {...form.getInputProps('game')}
        />
        <TextArea
          withAsterisk
          autosize
          minRows={3}
          maxRows={10}
          label={t(`${page}.form.description`, {})}
          description={t(`${page}.form.descriptionHint`, {})}
          {...form.getInputProps('description')}
        />
        <ServerMultiSelect
          label={t(`${page}.form.servers`, {})}
          description={t(`${page}.form.serversHint`, {})}
          value={servers}
          onChange={(selected) => setServers(selected.slice(0, 10))}
        />

        {refusal && (
          <div className='flex flex-col gap-2' role='alert'>
            <span className='theme-eco-title'>
              {refusal.guildName
                ? t(`${page}.refusal.titleServer`, { server: refusal.guildName })
                : t(`${page}.refusal.title`, {})}
            </span>
            {refusal.problems.map((problem) => (
              <p key={problem} className='theme-eco-text theme-eco-danger'>
                {problemText(translations, problem, partners, appName, {
                  members: refusal.members,
                  channel: refusal.channel?.name,
                  applyAfter: refusal.applyAfter,
                })}
              </p>
            ))}
            {refusal.problems.includes('bot_missing') && refusal.botInviteUrl && (
              <div>
                <ExternalButton href={refusal.botInviteUrl} filled={false}>
                  {t(`${page}.requirements.addBot`, { bot: partners.botName })}
                </ExternalButton>
              </div>
            )}
          </div>
        )}

        <div>
          <Button type='submit' loading={loading}>
            {t(`${page}.form.submit`, {})}
          </Button>
        </div>
      </form>
    </section>
  );
}

function Partnership({
  partners,
  application,
  onCheck,
  checking,
}: {
  partners: Partners;
  application: Application;
  onCheck: () => void;
  checking: boolean;
}) {
  const translations = useTranslations();
  const { t, language } = translations;
  const appName = useGlobalStore((state) => state.settings.app.name);
  const failing = application.status === 'approved' && application.suspendAfter !== null;
  const problems = application.problems.map((problem) =>
    problemText(translations, problem, partners, appName, {
      members: application.members,
      channel: application.channel?.name,
    }),
  );

  return (
    <section className='flex flex-col gap-3'>
      <div className='theme-eco-card'>
        <div className='flex flex-col gap-1'>
          <div className='flex flex-wrap items-center gap-2'>
            <h3 className='theme-eco-heading'>{application.guildName}</h3>
            <Badge color={application.status === 'approved' && !failing ? 'green' : 'yellow'} variant='light'>
              {t(`${page}.status.${application.status}`, {})}
            </Badge>
          </div>
          <span className='theme-eco-value'>
            {t(`${page}.partnership.monthly`, { credits: (application.monthlyCredits ?? 0).toLocaleString() })}
          </span>
          {application.status === 'approved' && application.nextPayment && (
            <p className='theme-eco-muted'>
              {t(`${page}.partnership.nextPayment`, { date: formatDate(application.nextPayment, language) })}
            </p>
          )}
        </div>

        {application.status === 'paused' && (
          <p className='theme-eco-text'>
            {application.reason
              ? t(`${page}.partnership.pausedReason`, { reason: application.reason })
              : t(`${page}.partnership.paused`, {})}
          </p>
        )}
        {(failing || application.status === 'suspended') && (
          <div className='flex flex-col gap-1' role='alert'>
            <p className='theme-eco-text'>
              {failing && application.suspendAfter
                ? t(`${page}.partnership.failing`, { date: formatDate(application.suspendAfter, language) })
                : t(`${page}.partnership.suspended`, {})}
            </p>
            {problems.map((problem) => (
              <p key={problem} className='theme-eco-text theme-eco-danger'>
                {problem}
              </p>
            ))}
          </div>
        )}

        {(failing || application.status === 'suspended') && (
          <div>
            <Button variant='default' loading={checking} onClick={onCheck}>
              {t(`${page}.partnership.check`, {})}
            </Button>
          </div>
        )}
      </div>

      <div className='flex flex-col gap-1'>
        <h4 className='theme-eco-title'>{t(`${page}.partnership.payments`, {})}</h4>
        <p className='theme-eco-muted'>
          {t(`${page}.partnership.paidTotal`, { credits: application.paidTotal.toLocaleString() })}
        </p>
      </div>
      <div className='theme-eco-list'>
        {application.payments.length === 0 ? (
          <p className='theme-eco-muted theme-eco-empty'>{t(`${page}.partnership.paymentsEmpty`, {})}</p>
        ) : (
          application.payments.map((payment) => (
            <Row
              key={payment.period}
              text={new Date(`${payment.period}-01T12:00:00Z`).toLocaleDateString(intlLocale(language), {
                year: 'numeric',
                month: 'long',
              })}
              hint={formatDate(payment.created, language)}
            >
              <span className='theme-eco-amount theme-eco-amount-in'>+{payment.credits.toLocaleString()}</span>
            </Row>
          ))
        )}
      </div>
    </section>
  );
}

function Pending({
  application,
  onWithdraw,
  withdrawing,
}: {
  application: Application;
  onWithdraw: () => void;
  withdrawing: boolean;
}) {
  const { t } = useTranslations();

  return (
    <section className='theme-eco-card'>
      <div className='flex flex-col gap-1'>
        <h3 className='theme-eco-heading'>{t(`${page}.pending.title`, {})}</h3>
        <p className='theme-eco-text max-w-2xl'>{t(`${page}.pending.body`, { server: application.guildName })}</p>
      </div>
      <div>
        <Button variant='default' loading={withdrawing} onClick={onWithdraw}>
          {t(`${page}.pending.withdraw`, {})}
        </Button>
      </div>
    </section>
  );
}

function History({ applications }: { applications: Application[] }) {
  const { t, language } = useTranslations();

  if (applications.length === 0) {
    return null;
  }

  return (
    <section className='flex flex-col gap-3'>
      <h3 className='theme-eco-heading'>{t(`${page}.history.title`, {})}</h3>
      <div className='theme-eco-list'>
        {applications.map((application) => (
          <Row
            key={application.uuid}
            text={application.guildName}
            hint={
              application.reason
                ? t(`${page}.history.reason`, { reason: application.reason })
                : formatDate(application.decided ?? application.created, language)
            }
          >
            <span className='theme-eco-muted'>{t(`${page}.status.${application.status}`, {})}</span>
          </Row>
        ))}
      </div>
    </section>
  );
}

function PartnersPage({ partners }: { partners: Partners }) {
  const translations = useTranslations();
  const { t } = translations;
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const open = useGlobalStore((state) => !!state.settings.partners?.open);
  const [busy, setBusy] = useState<string | null>(null);

  const partnerships = partners.applications.filter((application) => PARTNERSHIPS.includes(application.status));
  const pending = partners.applications.find((application) => application.status === 'pending');
  const earlier = partners.applications.filter(
    (application) => application.status !== 'pending' && !PARTNERSHIPS.includes(application.status),
  );

  const update = (after: Partners) => queryClient.setQueryData(queryKeys.user.partners(), after);

  const withdraw = () => {
    setBusy('withdraw');
    withdrawPartner()
      .then((after) => {
        update(after);
        addToast(t(`${page}.pending.withdrawn`, {}), 'success');
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setBusy(null));
  };

  const check = (application: Application) => {
    setBusy(application.uuid);
    checkPartnership(application.uuid)
      .then((after) => {
        update(after);
        const now = after.applications.find((entry) => entry.uuid === application.uuid);
        addToast(
          now && now.problems.length === 0
            ? t(`${page}.partnership.checkPassed`, {})
            : t(`${page}.partnership.checkFailed`, {}),
          now && now.problems.length === 0 ? 'success' : 'error',
        );
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setBusy(null));
  };

  return (
    <div className='flex flex-col gap-6'>
      {partnerships.map((application) => (
        <Partnership
          key={application.uuid}
          partners={partners}
          application={application}
          onCheck={() => check(application)}
          checking={busy === application.uuid}
        />
      ))}

      {pending && <Pending application={pending} onWithdraw={withdraw} withdrawing={busy === 'withdraw'} />}

      {!pending && (
        <section className='theme-eco-card'>
          <div className='flex flex-col gap-1'>
            <h3 className='theme-eco-heading'>{t(`${page}.intro.heading`, {})}</h3>
            <p className='theme-eco-text max-w-2xl'>
              {t(`${page}.intro.body`, {
                app: appName,
                members: partners.minMembers.toLocaleString(),
                credits: partners.maxCredits.toLocaleString(),
              })}
            </p>
            <p className='theme-eco-muted max-w-2xl'>
              {partners.graceDays === 0
                ? t(`${page}.intro.reviewNow`, {})
                : partners.graceDays === 1
                  ? t(`${page}.intro.reviewDay`, { days: partners.graceDays })
                  : t(`${page}.intro.review`, { days: partners.graceDays })}
            </p>
          </div>
          {!open && <p className='theme-eco-text'>{t(`${page}.closed`, {})}</p>}
          {open && !partners.canApply && partners.applyAfter && (
            <p className='theme-eco-text'>
              {problemText(translations, 'cooldown', partners, appName, { applyAfter: partners.applyAfter })}
            </p>
          )}
        </section>
      )}

      {!pending && open && <Requirements partners={partners} />}

      {!pending && open && partners.canApply && partners.discord && (
        <ApplyForm partners={partners} onApplied={update} />
      )}

      <History applications={earlier} />
    </div>
  );
}

export default function DashboardPartners() {
  const { t } = useTranslations();

  const partners = useResource({
    queryKey: queryKeys.user.partners(),
    queryFn: getPartners,
  });

  return (
    <AccountContentContainer title={t(`${page}.title`, {})}>
      <ResourceView resource={partners}>{(partners) => <PartnersPage partners={partners} />}</ResourceView>
    </AccountContentContainer>
  );
}
