// plugged.host: Discord rewards, laid out like the Pelican panel's Discord rewards tab: what Discord
// earned with the account's link and membership, inviting friends, boosting, and the history
// (docs/features/discord.md). At most one filled button: linking while there is no link, joining
// the server while linked but not in it.
import { Button as MantineButton } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { ReactNode, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import checkDiscordRewards from '@/api/me/discord/checkDiscordRewards.ts';
import getDiscordRewards from '@/api/me/discord/getDiscordRewards.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import TransactionRows from '@/elements/economy/TransactionRows.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { formatTimestamp } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { discordRewardsSchema } from '@/lib/schemas/user/discord.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type Rewards = z.infer<typeof discordRewardsSchema>;

const page = 'pages.account.discord';

// a full-width row: what it is and a line about it on the left, the number on the right
function Row({ text, hint, children }: { text: string; hint?: ReactNode; children: ReactNode }) {
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

function Amount({ tone, children }: { tone: 'in' | 'out' | 'danger'; children: ReactNode }) {
  return (
    <span className={`theme-eco-amount ${tone === 'danger' ? 'theme-eco-danger' : `theme-eco-amount-${tone}`}`}>
      {children}
    </span>
  );
}

// a link to the Discord server or noprofit.space, in a new tab
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

function Unlinked({ rewards }: { rewards: Rewards }) {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const appName = useGlobalStore((state) => state.settings.app.name);

  return (
    <section className='theme-eco-card'>
      <div className='flex flex-col gap-1'>
        <h3 className='theme-eco-heading'>
          {rewards.pays ? t(`${page}.unlinked.heading`, {}) : t(`${page}.unlinked.headingNoCredits`, {})}
        </h3>
        <p className='theme-eco-text max-w-2xl'>
          {rewards.pays
            ? t(`${page}.unlinked.body`, { app: appName, credits: rewards.rules.creditsPerInvite.toLocaleString() })
            : t(`${page}.unlinked.bodyNoCredits`, { app: appName })}
        </p>
      </div>

      <div className='flex flex-wrap gap-3'>
        {rewards.linkUrl ? (
          <ExternalButton href={rewards.linkUrl} filled>
            {t(`${page}.unlinked.linkNoprofit`, {})}
          </ExternalButton>
        ) : (
          <Button onClick={() => navigate('/account/oauth-links')}>{t(`${page}.unlinked.link`, {})}</Button>
        )}
        {rewards.inviteUrl && (
          <ExternalButton href={rewards.inviteUrl} filled={false}>
            {t(`${page}.join`, {})}
          </ExternalButton>
        )}
      </div>
    </section>
  );
}

function Linked({
  rewards,
  account,
  onCheck,
  checking,
}: {
  rewards: Rewards;
  account: NonNullable<Rewards['account']>;
  onCheck: () => void;
  checking: boolean;
}) {
  const { t } = useTranslations();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const { rules, invites, boosts } = rewards;
  const name = account.username ?? account.id;
  const twoBoostCredits = Math.max(0, rules.boostTwoCredits - rules.boostOneCredits);

  const tier = [
    rules.tier.cpu > 0 ? t(`${page}.tier.cpu`, { cpu: rules.tier.cpu }) : null,
    rules.tier.memory > 0 ? t(`${page}.tier.memory`, { memory: bytesToString(mbToBytes(rules.tier.memory)) }) : null,
    rules.tier.disk > 0 ? t(`${page}.tier.disk`, { disk: bytesToString(mbToBytes(rules.tier.disk)) }) : null,
  ].filter((part) => part !== null);

  const membership =
    rewards.inGuild === null ? (
      t(`${page}.notChecked`, {})
    ) : rewards.inGuild ? (
      t(`${page}.inServer`, {})
    ) : (
      <span className='theme-eco-danger'>{t(`${page}.notInServer`, {})}</span>
    );

  const nextPayout =
    invites.waiting > 0 && invites.nextPayout
      ? invites.nextPayout.getTime() <= Date.now()
        ? t(`${page}.invites.nextSoon`, {})
        : t(`${page}.invites.next`, { time: formatTimestamp(invites.nextPayout) })
      : t(`${page}.invites.waitingNone`, {});

  const levels = [
    { boosts: 1, credits: rules.boostOneCredits, total: rules.boostOneCredits },
    { boosts: 2, credits: twoBoostCredits, total: rules.boostOneCredits + twoBoostCredits },
  ];

  return (
    <div className='flex flex-col gap-6'>
      <section className='theme-eco-card'>
        <div className='flex flex-col gap-1'>
          {rewards.pays ? (
            <>
              <span className='theme-eco-heading'>{t(`${page}.earned`, {})}</span>
              <span className='theme-eco-value'>{rewards.earned.toLocaleString()}</span>
              <p className='theme-eco-muted'>
                {t(`${page}.linkedAs`, { name })} · {membership}
              </p>
            </>
          ) : (
            <>
              <span className='theme-eco-heading'>{t(`${page}.linkedAs`, { name })}</span>
              <p className='theme-eco-muted'>{membership}</p>
            </>
          )}
          {tier.length > 0 && (
            <p className='theme-eco-muted'>{t(`${page}.tier.text`, { resources: tier.join(', ') })}</p>
          )}
        </div>

        <div className='flex flex-wrap gap-3'>
          {rewards.inviteUrl && (
            <ExternalButton href={rewards.inviteUrl} filled={rewards.inGuild === false}>
              {rewards.inGuild ? t(`${page}.open`, {}) : t(`${page}.join`, {})}
            </ExternalButton>
          )}
          <Button variant='default' loading={checking} onClick={onCheck}>
            {t(`${page}.check`, {})}
          </Button>
        </div>
      </section>

      <section className='flex flex-col gap-3'>
        <div className='flex flex-col gap-1'>
          <h3 className='theme-eco-heading'>{t(`${page}.invites.title`, {})}</h3>
          <p className='theme-eco-muted max-w-2xl'>
            {rewards.pays
              ? rules.inviteMaturityHours > 0
                ? t(rules.inviteMaturityHours === 1 ? `${page}.invites.ruleHour` : `${page}.invites.rule`, {
                    credits: rules.creditsPerInvite.toLocaleString(),
                    hours: rules.inviteMaturityHours,
                  })
                : t(`${page}.invites.ruleNow`, { credits: rules.creditsPerInvite.toLocaleString() })
              : t(`${page}.invites.notPaid`, {})}
            {rules.minAccountAgeDays > 0 &&
              ` ${t(rules.minAccountAgeDays === 1 ? `${page}.invites.ageDay` : `${page}.invites.age`, {
                days: rules.minAccountAgeDays,
              })}`}
          </p>
        </div>

        <div className='theme-eco-list'>
          <Row
            text={t(`${page}.invites.paid`, {})}
            hint={
              invites.paid === 0
                ? t(`${page}.invites.friendsNone`, {})
                : invites.paid === 1
                  ? t(`${page}.invites.friend`, {})
                  : t(`${page}.invites.friends`, { count: invites.paid.toLocaleString() })
            }
          >
            {rewards.pays ? (
              <Amount tone='in'>+{invites.credits.toLocaleString()}</Amount>
            ) : (
              <Amount tone='out'>{invites.paid.toLocaleString()}</Amount>
            )}
          </Row>
          <Row text={t(`${page}.invites.waiting`, {})} hint={nextPayout}>
            <Amount tone='out'>{invites.waiting.toLocaleString()}</Amount>
          </Row>
          {invites.left > 0 && (
            <Row
              text={t(`${page}.invites.left`, {})}
              hint={
                rules.inviteClawback && rewards.pays
                  ? t(`${page}.invites.leftClawback`, {})
                  : t(`${page}.invites.leftKept`, {})
              }
            >
              <Amount tone='out'>{invites.left.toLocaleString()}</Amount>
            </Row>
          )}
          {invites.tooNew > 0 && (
            <Row
              text={t(`${page}.invites.tooNew`, {})}
              hint={
                rules.minAccountAgeDays > 0
                  ? t(rules.minAccountAgeDays === 1 ? `${page}.invites.tooNewHintDay` : `${page}.invites.tooNewHint`, {
                      days: rules.minAccountAgeDays,
                    })
                  : t(`${page}.invites.tooNewOwn`, {})
              }
            >
              <Amount tone='out'>{invites.tooNew.toLocaleString()}</Amount>
            </Row>
          )}
          {invites.owed > 0 && (
            <Row text={t(`${page}.invites.owed`, {})} hint={t(`${page}.invites.owedHint`, {})}>
              <Amount tone='danger'>−{invites.owed.toLocaleString()}</Amount>
            </Row>
          )}
        </div>

        <p className='theme-eco-small max-w-2xl'>{t(`${page}.invites.how`, { app: appName })}</p>
      </section>

      {/* with noprofit.space its Discord page has the boosts (Pelican's "Boost our servers") */}
      {rewards.source === 'noprofit' && rewards.noprofitUrl && (
        <section className='flex flex-col gap-3'>
          <div className='flex flex-col gap-1'>
            <h3 className='theme-eco-heading'>{t(`${page}.noprofit.title`, {})}</h3>
            <p className='theme-eco-muted max-w-2xl'>{t(`${page}.noprofit.text`, {})}</p>
          </div>
          <div className='flex flex-wrap gap-3'>
            <ExternalButton href={rewards.noprofitUrl} filled={false}>
              {t(`${page}.noprofit.open`, {})}
            </ExternalButton>
          </div>
        </section>
      )}

      {boosts && (
        <section className='flex flex-col gap-3'>
          <div className='flex flex-col gap-1'>
            <h3 className='theme-eco-heading'>{t(`${page}.boosts.title`, {})}</h3>
            <p className='theme-eco-muted'>
              {boosts.level >= 2
                ? t(`${page}.boosts.boostingTwo`, {})
                : boosts.level === 1
                  ? t(`${page}.boosts.boosting`, {})
                  : t(`${page}.boosts.notBoosting`, {})}
            </p>
          </div>

          {rewards.pays && (
            <>
              <div className='theme-eco-list'>
                {levels.map((level) => (
                  <Row
                    key={level.boosts}
                    text={level.boosts === 1 ? t(`${page}.boosts.levelOne`, {}) : t(`${page}.boosts.levelTwo`, {})}
                    hint={
                      level.boosts === 1
                        ? t(`${page}.boosts.levelOneCredits`, { credits: level.credits.toLocaleString() })
                        : t(`${page}.boosts.levelTwoCredits`, {
                            credits: level.credits.toLocaleString(),
                            total: level.total.toLocaleString(),
                          })
                    }
                  >
                    {boosts.paidLevels.includes(level.boosts) ? (
                      <Badge color='green' variant='light'>
                        {t(`${page}.boosts.received`, {})}
                      </Badge>
                    ) : (
                      <Amount tone='out'>+{level.credits.toLocaleString()}</Amount>
                    )}
                  </Row>
                ))}
              </div>

              <p className='theme-eco-small'>{t(`${page}.boosts.once`, {})}</p>
            </>
          )}
        </section>
      )}

      {rewards.source === 'panel' && rewards.pays && (
        <section className='flex flex-col gap-3'>
          <h3 className='theme-eco-heading'>{t(`${page}.history`, {})}</h3>

          <div className='theme-eco-list'>
            {rewards.history.length === 0 ? (
              <p className='theme-eco-muted theme-eco-empty'>{t(`${page}.historyEmpty`, {})}</p>
            ) : (
              <TransactionRows transactions={rewards.history} variant='ledger' />
            )}
          </div>
        </section>
      )}
    </div>
  );
}

export default function DashboardDiscord() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [checking, setChecking] = useState(false);

  const rewards = useResource({
    queryKey: queryKeys.user.discord(),
    queryFn: getDiscordRewards,
  });

  const check = (before: Rewards) => {
    setChecking(true);
    checkDiscordRewards()
      .then((after) => {
        queryClient.setQueryData(queryKeys.user.discord(), after);
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
        const added = after.earned - before.earned;
        addToast(
          added > 0
            ? t(`${page}.toast.paid`, { credits: added.toLocaleString() })
            : after.pays
              ? t(`${page}.toast.nothing`, {})
              : t(`${page}.toast.checked`, {}),
          'success',
        );
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setChecking(false));
  };

  return (
    <AccountContentContainer title={t(`${page}.title`, {})}>
      <ResourceView resource={rewards}>
        {(rewards) =>
          rewards.account ? (
            <Linked rewards={rewards} account={rewards.account} onCheck={() => check(rewards)} checking={checking} />
          ) : (
            <Unlinked rewards={rewards} />
          )
        }
      </ResourceView>
    </AccountContentContainer>
  );
}
