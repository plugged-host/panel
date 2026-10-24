// plugged.host: the memberships, laid out like the Pelican panel's Plus page: the two plans side
// by side, what each adds over the free tier, and the questions people ask before buying
// (docs/features/economy.md).
import { useQueryClient } from '@tanstack/react-query';
import { Fragment, ReactNode, useState } from 'react';
import { Link } from 'react-router';
import { z } from 'zod';
import buyPlus from '@/api/me/economy/buyPlus.ts';
import buyPlusPlus from '@/api/me/economy/buyPlusPlus.ts';
import getMembership from '@/api/me/economy/getMembership.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { formatDateTime } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { to } from '@/lib/routes.ts';
import { userEconomySchema } from '@/lib/schemas/user/economy.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type Economy = z.infer<typeof userEconomySchema>;
type Offer = Economy['plusplusOffers'][number];
type DevboxSize = NonNullable<Economy['devbox']>['standard'];

const page = 'pages.account.membership';

/** The three strings that spell out a devbox's resources. */
type SizeKey =
  | `${typeof page}.perks.devbox`
  | `${typeof page}.perks.devboxPlusplus`
  | `${typeof page}.compare.cells.devbox`;

function PlanCard({
  name,
  pitch,
  price,
  term,
  perks,
  children,
}: {
  name: string;
  pitch: string;
  price: string;
  term?: string;
  perks: string[];
  children: ReactNode;
}) {
  return (
    <section className='theme-eco-card'>
      <div className='flex flex-col gap-1'>
        <h3 className='theme-eco-heading'>{name}</h3>
        <p className='theme-eco-muted'>{pitch}</p>
      </div>

      <div className='flex flex-col gap-0.5'>
        <span className='theme-plus-price'>{price}</span>
        {term && <span className='theme-eco-small'>{term}</span>}
      </div>

      <div className='theme-plus-plan-body'>
        <ul className='theme-plus-perks'>
          {perks.map((perk) => (
            <li key={perk}>{perk}</li>
          ))}
        </ul>
      </div>

      <div className='flex flex-col gap-2'>{children}</div>
    </section>
  );
}

export default function DashboardMembership() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const names = useGlobalStore((state) => state.settings.economy);

  const membership = useResource({
    queryKey: queryKeys.user.membership(),
    queryFn: getMembership,
  });

  const [buying, setBuying] = useState<{ kind: 'plus' } | { kind: 'plusplus'; offer: Offer } | null>(null);

  const done = (economy: Economy) => {
    queryClient.setQueryData(queryKeys.user.membership(), economy);
    queryClient.invalidateQueries({ queryKey: queryKeys.user.tokens() });
    queryClient.invalidateQueries({ queryKey: queryKeys.user.store() });
    queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
    queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.createOptions() });
    queryClient.invalidateQueries({ queryKey: queryKeys.user.ads() }); // a membership changes the ads
    setBuying(null);
  };

  const doBuy = async () => {
    if (!buying) return;

    if (buying.kind === 'plus') {
      const economy = await buyPlus();
      addToast(t(`${page}.toast.plus`, { name: economy.plusName }), 'success');
      done(economy);
    } else {
      const economy = await buyPlusPlus(buying.offer.pay);
      addToast(t(`${page}.toast.plusplus`, { name: economy.plusplusName }), 'success');
      done(economy);
    }
  };

  return (
    <AccountContentContainer
      title={names ? t(`${page}.pageTitle`, { app: appName, name: names.plusName }) : t(`${page}.title`, {})}
      subtitle={t(`${page}.subtitle`, {})}
    >
      <ResourceView resource={membership}>
        {(economy) => {
          const hasPlus = economy.plusSince !== null;
          const tokensOffer = economy.plusplusOffers.find((offer) => offer.pay === 'tokens');
          const creditsOffer = economy.plusplusOffers.find((offer) => offer.pay === 'credits');
          const pricedOffer = tokensOffer ?? economy.plusplusOffers[0];
          // "a week, or credits for two weeks", as the Pelican panel words it
          const duration = (days: number) =>
            days % 7 === 0
              ? days === 7
                ? t(`${page}.duration.week`, {})
                : days === 14
                  ? t(`${page}.duration.twoWeeks`, {})
                  : t(`${page}.duration.weeks`, { count: days / 7 })
              : days === 1
                ? t(`${page}.duration.day`, {})
                : t(`${page}.duration.days`, { count: days });
          const plusplusTerm =
            tokensOffer && creditsOffer
              ? t(`${page}.plusplus.term`, { tokens: duration(tokensOffer.days), credits: duration(creditsOffer.days) })
              : pricedOffer
                ? t(`${page}.plusplus.termTokens`, { tokens: duration(pricedOffer.days) })
                : undefined;
          const balanceOf = (pay: Offer['pay']) => (pay === 'tokens' ? economy.tokens : economy.credits);
          const size = (key: SizeKey, value: DevboxSize) =>
            t(key, {
              cpu: (value.cpu / 100).toLocaleString(),
              memory: bytesToString(mbToBytes(value.memory)),
              disk: bytesToString(mbToBytes(value.disk)),
            });
          const cpuCell = (cores: number) =>
            cores > 0
              ? t(`${page}.compare.cells.cpuBurst`, { cpu: (cores * 100).toLocaleString() })
              : t(`${page}.compare.cells.cpu`, {});

          const rows: { label: string; free: string; plus: string; plusplus: string }[] = [
            {
              label: t(`${page}.compare.rows.locations`, {}),
              free: t(`${page}.compare.cells.locationsFree`, {}),
              plus: t(`${page}.compare.cells.locationsPlus`, { plus: economy.plusName }),
              plusplus: t(`${page}.compare.cells.locationsPlusplus`, { plus: economy.plusName }),
            },
            {
              label: t(`${page}.compare.rows.queue`, {}),
              free: t(`${page}.compare.cells.queueFree`, {}),
              plus: t(`${page}.compare.cells.queuePlus`, {}),
              plusplus: t(`${page}.compare.cells.queuePlusplus`, {}),
            },
            {
              label: t(`${page}.compare.rows.gameAds`, {}),
              free: t(`${page}.compare.cells.gameAdsFree`, {}),
              plus: t(`${page}.compare.cells.none`, {}),
              plusplus: t(`${page}.compare.cells.none`, {}),
            },
            {
              label: t(`${page}.compare.rows.panelAds`, {}),
              free: t(`${page}.compare.cells.yes`, {}),
              plus: t(`${page}.compare.cells.yes`, {}),
              plusplus: t(`${page}.compare.cells.no`, {}),
            },
            {
              label: t(`${page}.compare.rows.cpu`, {}),
              free: t(`${page}.compare.cells.cpu`, {}),
              plus: cpuCell(economy.plusBurstCores),
              plusplus: cpuCell(economy.plusplusBurstCores),
            },
            {
              label: t(`${page}.compare.rows.ai`, {}),
              free: t(`${page}.compare.cells.aiFree`, {}),
              plus:
                economy.neoPlusPercent !== null && economy.neoPlusPercent > 0
                  ? t(`${page}.compare.cells.percent`, { percent: economy.neoPlusPercent.toLocaleString() })
                  : t(`${page}.compare.cells.aiFree`, {}),
              plusplus:
                economy.neoPlusplusPercent !== null && economy.neoPlusplusPercent > 0
                  ? t(`${page}.compare.cells.percent`, { percent: economy.neoPlusplusPercent.toLocaleString() })
                  : t(`${page}.compare.cells.aiFree`, {}),
            },
          ];

          if (economy.devbox) {
            rows.push({
              label: t(`${page}.compare.rows.devbox`, {}),
              free: economy.devbox.membersOnly
                ? t(`${page}.compare.cells.no`, {})
                : size(`${page}.compare.cells.devbox`, economy.devbox.standard),
              plus: size(`${page}.compare.cells.devbox`, economy.devbox.standard),
              plusplus: size(`${page}.compare.cells.devbox`, economy.devbox.premium),
            });
          }

          const plusPerks = [
            t(`${page}.perks.plusLocations`, { name: economy.plusName }),
            t(`${page}.perks.priority`, {}),
            t(`${page}.perks.noGameAds`, {}),
          ];
          if (economy.plusBurstCores > 0) {
            plusPerks.push(t(`${page}.perks.burst`, { cpu: (economy.plusBurstCores * 100).toLocaleString() }));
          }
          if (economy.neoPlusPercent !== null && economy.neoPlusPercent > 0) {
            plusPerks.push(t(`${page}.perks.neoPlus`, { percent: economy.neoPlusPercent.toLocaleString() }));
          }
          if (economy.devbox) {
            plusPerks.push(size(`${page}.perks.devbox`, economy.devbox.standard));
          }

          const plusplusPerks = [
            t(`${page}.perks.everythingInPlus`, { name: economy.plusName }),
            t(`${page}.perks.premiumLocations`, {}),
            t(`${page}.perks.noAds`, {}),
            t(`${page}.perks.frontOfQueue`, { name: economy.plusName }),
          ];
          if (economy.plusplusBurstCores > 0) {
            plusplusPerks.push(t(`${page}.perks.burst`, { cpu: (economy.plusplusBurstCores * 100).toLocaleString() }));
          }
          if (economy.neoPlusplusPercent !== null && economy.neoPlusplusPercent > 0) {
            plusplusPerks.push(
              t(`${page}.perks.neoPlusplus`, { percent: economy.neoPlusplusPercent.toLocaleString() }),
            );
          }
          if (economy.devbox) {
            plusplusPerks.push(size(`${page}.perks.devboxPlusplus`, economy.devbox.premium));
          }

          const faq: { question: string; answer: string }[] = [
            {
              question: t(`${page}.faq.once.question`, { plus: economy.plusName }),
              answer: t(`${page}.faq.once.answer`, { plus: economy.plusName, plusplus: economy.plusplusName }),
            },
            {
              question: t(`${page}.faq.expiry.question`, { plusplus: economy.plusplusName }),
              answer: t(`${page}.faq.expiry.answer`, { plus: economy.plusName, plusplus: economy.plusplusName }),
            },
            { question: t(`${page}.faq.tokens.question`, {}), answer: t(`${page}.faq.tokens.answer`, {}) },
          ];
          if (creditsOffer) {
            faq.push({
              question: t(`${page}.faq.credits.question`, {}),
              answer: t(`${page}.faq.credits.answer`, {
                plus: economy.plusName,
                plusplus: economy.plusplusName,
                duration: duration(creditsOffer.days),
              }),
            });
          }

          return (
            <Stack gap='lg'>
              <ConfirmationModal
                opened={!!buying}
                onClose={() => setBuying(null)}
                title={t(`${page}.modal.title`, {})}
                confirm={t(`${page}.modal.confirm`, {})}
                confirmColor='blue'
                onConfirmed={doBuy}
              >
                {buying?.kind === 'plus' &&
                  t(`${page}.modal.plus`, {
                    name: economy.plusName,
                    cost: economy.plusCost.toLocaleString(),
                  }).md()}
                {buying?.kind === 'plusplus' &&
                  t(`${page}.modal.plusplus.${buying.offer.pay}`, {
                    name: economy.plusplusName,
                    days: buying.offer.days,
                    cost: buying.offer.cost.toLocaleString(),
                  }).md()}
              </ConfirmationModal>

              {!economy.membershipEnabled ? (
                <p className='theme-eco-muted'>{t(`${page}.unavailable`, { name: economy.plusName })}</p>
              ) : (
                <>
                  {economy.plusplusUntil ? (
                    <Notification
                      className='theme-callout'
                      color='green'
                      title={t(`${page}.plusplusActive.heading`, { name: economy.plusplusName })}
                    >
                      {t(`${page}.plusplusActive.body`, { until: formatDateTime(economy.plusplusUntil) })}
                    </Notification>
                  ) : hasPlus ? (
                    <Notification
                      className='theme-callout'
                      color='green'
                      title={t(`${page}.owned.heading`, { app: appName, name: economy.plusName })}
                    >
                      {t(`${page}.owned.body`, { name: economy.plusName, plusplus: economy.plusplusName })}
                    </Notification>
                  ) : null}

                  <div className='theme-plus-plans'>
                    <PlanCard
                      name={t(`${page}.pageTitle`, { app: appName, name: economy.plusName })}
                      pitch={t(`${page}.plus.pitch`, {})}
                      price={t(`${page}.cost.tokens`, { cost: economy.plusCost.toLocaleString() })}
                      term={t(`${page}.plus.term`, {})}
                      perks={plusPerks}
                    >
                      {hasPlus ? (
                        <Badge color='green' variant='light' className='self-start'>
                          {t(`${page}.plus.owned`, {})}
                        </Badge>
                      ) : (
                        <>
                          <Button
                            className='w-full'
                            disabled={!economy.tokensEnabled || economy.tokens < economy.plusCost}
                            onClick={() => setBuying({ kind: 'plus' })}
                          >
                            {t(`${page}.plus.get`, { name: economy.plusName })}
                          </Button>

                          {economy.tokens < economy.plusCost && (
                            <p className='theme-eco-small'>
                              {t(`${page}.balance`, { balance: economy.tokens.toLocaleString() })}{' '}
                              <Link to={to('/tokens', '/account')} className='theme-eco-link'>
                                {t(`${page}.earnLink`, {})}
                              </Link>
                            </p>
                          )}
                        </>
                      )}
                    </PlanCard>

                    <PlanCard
                      name={t(`${page}.pageTitle`, { app: appName, name: economy.plusplusName })}
                      pitch={t(`${page}.plusplus.pitch`, {})}
                      price={
                        pricedOffer
                          ? t(`${page}.cost.${pricedOffer.pay}`, { cost: pricedOffer.cost.toLocaleString() })
                          : t(`${page}.plusplus.notSold`, {})
                      }
                      term={plusplusTerm}
                      perks={plusplusPerks}
                    >
                      {economy.plusplusOffers.map((offer) => {
                        const credits = offer.pay === 'credits';

                        return (
                          <Button
                            key={offer.pay}
                            className='theme-plus-offer w-full'
                            variant={!credits && hasPlus ? 'filled' : 'default'}
                            disabled={!hasPlus || balanceOf(offer.pay) < offer.cost}
                            onClick={() => setBuying({ kind: 'plusplus', offer })}
                          >
                            <span className='font-semibold'>{t(`${page}.days`, { days: offer.days })}</span>
                            <span className='theme-plus-offer-cost'>
                              {t(`${page}.cost.${offer.pay}`, { cost: offer.cost.toLocaleString() })}
                            </span>
                          </Button>
                        );
                      })}

                      <p className='theme-eco-small'>
                        {!hasPlus
                          ? t(`${page}.plusplus.needsPlus`, {
                              app: appName,
                              name: economy.plusplusName,
                              plus: economy.plusName,
                            })
                          : economy.plusplusOffers.length === 0
                            ? t(`${page}.plusplus.notSold`, {})
                            : t(`${page}.plusplus.extendHint`, {})}
                      </p>
                    </PlanCard>
                  </div>

                  <section className='flex flex-col gap-3'>
                    <h3 className='theme-eco-heading'>{t(`${page}.compare.heading`, {})}</h3>

                    <div className='theme-eco-list'>
                      <div className='theme-plus-compare' role='table' aria-label={t(`${page}.compare.heading`, {})}>
                        <div className='theme-plus-head' role='columnheader' />
                        <div className='theme-plus-head' role='columnheader'>
                          {t(`${page}.compare.free`, {})}
                        </div>
                        <div className='theme-plus-head' role='columnheader'>
                          {economy.plusName}
                        </div>
                        <div className='theme-plus-head' role='columnheader'>
                          {economy.plusplusName}
                        </div>

                        {rows.map((row) => (
                          <Fragment key={row.label}>
                            <div className='theme-plus-rowhead' role='rowheader'>
                              {row.label}
                            </div>
                            <div role='cell'>{row.free}</div>
                            <div role='cell'>{row.plus}</div>
                            <div role='cell'>{row.plusplus}</div>
                          </Fragment>
                        ))}
                      </div>
                    </div>
                  </section>

                  <section className='flex flex-col gap-3'>
                    <h3 className='theme-eco-heading'>{t(`${page}.faq.heading`, {})}</h3>

                    <div className='theme-eco-list theme-plus-faq'>
                      {faq.map((entry) => (
                        <details key={entry.question}>
                          <summary>{entry.question}</summary>
                          <p className='theme-eco-text'>{entry.answer}</p>
                        </details>
                      ))}
                    </div>
                  </section>
                </>
              )}
            </Stack>
          );
        }}
      </ResourceView>
    </AccountContentContainer>
  );
}
