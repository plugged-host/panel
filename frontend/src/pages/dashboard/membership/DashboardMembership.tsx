// plugged.host: the memberships, laid out like the Pelican panel's Plus page: the two plans side
// by side (each bought by the month, neither needing the other), what each adds over the free
// tier, and the questions people ask before buying (docs/features/economy.md).
// An offer the balance does not cover says how much is missing and leads to the Tokens page; a
// membership that runs can renew by itself; a membership can be bought for someone else; a sale
// that covers the memberships shows a banner and struck-through prices, as in the store.
import { useQueryClient } from '@tanstack/react-query';
import { FormEvent, Fragment, ReactNode, useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { z } from 'zod';
import { getHttpStatus, httpErrorToHuman } from '@/api/axios.ts';
import buyPlus from '@/api/me/economy/buyPlus.ts';
import buyPlusPlus from '@/api/me/economy/buyPlusPlus.ts';
import getMembership from '@/api/me/economy/getMembership.ts';
import giftMembership from '@/api/me/economy/giftMembership.ts';
import setMembershipAutoRenew from '@/api/me/economy/setMembershipAutoRenew.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { formatDateTime } from '@/lib/format/time.ts';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { to } from '@/lib/routes.ts';
import { membershipKindSchema, userEconomySchema } from '@/lib/schemas/user/economy.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type Economy = z.infer<typeof userEconomySchema>;
type Offer = Economy['plusplusOffers'][number];
type MembershipKind = z.infer<typeof membershipKindSchema>;
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
  was,
  term,
  perks,
  children,
}: {
  name: string;
  pitch: string;
  price: string;
  /** The price without the sale, struck through in front of it while one runs. */
  was?: string;
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
        <span className='theme-plus-price'>
          {was && (
            <s className='theme-eco-was' aria-hidden='true'>
              {was}
            </s>
          )}
          {price}
        </span>
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
  const { t, tItem, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const names = useGlobalStore((state) => state.settings.economy);

  const membership = useResource({
    queryKey: queryKeys.user.membership(),
    queryFn: getMembership,
  });

  const [buying, setBuying] = useState<{ kind: 'plus'; price: number } | { kind: 'plusplus'; offer: Offer } | null>(
    null,
  );
  const [switching, setSwitching] = useState(false);
  const [giftTo, setGiftTo] = useState('');
  const [giftKind, setGiftKind] = useState<MembershipKind>('plus');
  const [gifting, setGifting] = useState<{
    recipient: string;
    kind: MembershipKind;
    name: string;
    price: number;
    days: number;
  } | null>(null);

  // a price that moved since the page was drawn (a sale started or ended) buys nothing: show the new ones
  const priceMoved = (error: unknown) => {
    if (getHttpStatus(error) === 409) {
      membership.invalidate();
      setBuying(null);
      setGifting(null);
    }
  };

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

    try {
      if (buying.kind === 'plus') {
        const economy = await buyPlus(buying.price);
        addToast(t(`${page}.toast.plus`, { name: economy.plusName }), 'success');
        done(economy);
      } else {
        const economy = await buyPlusPlus(buying.offer.pay, buying.offer.cost);
        addToast(t(`${page}.toast.plusplus`, { name: economy.plusplusName }), 'success');
        done(economy);
      }
    } catch (error) {
      priceMoved(error);
      throw error;
    }
  };

  const doGift = async () => {
    if (!gifting) return;

    try {
      const economy = await giftMembership(gifting.recipient, gifting.kind, gifting.price);
      addToast(t(`${page}.gift.toast`, { name: gifting.name, recipient: gifting.recipient }), 'success');
      queryClient.setQueryData(queryKeys.user.membership(), economy);
      queryClient.invalidateQueries({ queryKey: queryKeys.user.tokens() });
      queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
      setGifting(null);
      setGiftTo('');
    } catch (error) {
      priceMoved(error);
      throw error;
    }
  };

  const toggleAutoRenew = async (kind: MembershipKind, name: string, enabled: boolean) => {
    setSwitching(true);
    try {
      const economy = await setMembershipAutoRenew(kind, enabled);
      queryClient.setQueryData(queryKeys.user.membership(), economy);
      addToast(t(enabled ? `${page}.autoRenew.on` : `${page}.autoRenew.off`, { name }), 'success');
    } catch (error) {
      addToast(httpErrorToHuman(error), 'error');
    } finally {
      setSwitching(false);
    }
  };

  return (
    <AccountContentContainer
      title={names ? t(`${page}.pageTitle`, { app: appName, name: names.plusName }) : t(`${page}.title`, {})}
      subtitle={membership.data?.autoRenewEnabled ? t(`${page}.subtitleAutoRenew`, {}) : t(`${page}.subtitle`, {})}
    >
      <ResourceView resource={membership}>
        {(economy) => {
          const hasPlus = economy.plusUntil !== null;
          const hasPlusplus = economy.plusplusUntil !== null;
          // the membership the page reports as running: the second one when both run
          const activeUntil = economy.plusplusUntil ?? economy.plusUntil;
          const tokensOffer = economy.plusplusOffers.find((offer) => offer.pay === 'tokens');
          const creditsOffer = economy.plusplusOffers.find((offer) => offer.pay === 'credits');
          const pricedOffer = tokensOffer ?? economy.plusplusOffers[0];
          // "a month", "a week, or credits for two weeks", as the Pelican panel words it
          const duration = (days: number) =>
            days === 30
              ? t(`${page}.duration.month`, {})
              : days % 7 === 0
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
          // one filled button: Plus to start with, then the second membership
          // (plugged.host: Plus that comes with the bundle's other panel counts as having Plus)
          const plusFilled = !hasPlus && !hasPlusplus && !economy.bundlePlusUntil;
          const plusShort = economy.tokens < economy.plusCost;
          const plusplusShort =
            economy.plusplusOffers.length > 0 &&
            economy.plusplusOffers.every((offer) => balanceOf(offer.pay) < offer.cost);
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
              plus: t(`${page}.compare.cells.no`, {}),
              plusplus: t(`${page}.compare.cells.no`, {}),
            },
          ];

          if (economy.waitingRoom) {
            rows.push({
              label: t(`${page}.compare.rows.waitingRoom`, {}),
              free: t(`${page}.compare.cells.yes`, {}),
              plus: t(`${page}.compare.cells.yes`, {}),
              plusplus: t(`${page}.compare.cells.no`, {}),
            });
          }

          rows.push(
            {
              label: t(`${page}.compare.rows.cpu`, {}),
              free: t(`${page}.compare.cells.cpu`, {}),
              plus: cpuCell(economy.plusBurstCores),
              plusplus: cpuCell(economy.plusplusBurstCores),
            },
            {
              label: t(`${page}.compare.rows.files`, {}),
              free: t(`${page}.compare.cells.filesFree`, {}),
              plus: t(`${page}.compare.cells.filesPlus`, {}),
              plusplus: t(`${page}.compare.cells.filesPlusplus`, {}),
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
          );

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

          if (economy.panelAccess) {
            rows.push({
              label: t(`${page}.compare.rows.panelAccess`, {}),
              free: t(`${page}.compare.cells.panelAccessFree`, {}),
              plus: t(`${page}.compare.cells.included`, {}),
              plusplus: t(`${page}.compare.cells.included`, {}),
            });
          }

          // plugged.host: the membership the second one includes on another panel (shared/src/membership_bundle.rs)
          if (economy.bundleName) {
            rows.push({
              label: economy.bundleName,
              free: t(`${page}.compare.cells.no`, {}),
              plus: t(`${page}.compare.cells.no`, {}),
              plusplus: t(`${page}.compare.cells.included`, {}),
            });
          }

          const plusPerks = [
            t(`${page}.perks.plusLocations`, { name: economy.plusName }),
            t(`${page}.perks.priority`, {}),
            t(`${page}.perks.noBannerAds`, {}),
          ];
          if (economy.plusBurstCores > 0) {
            plusPerks.push(t(`${page}.perks.burst`, { cpu: (economy.plusBurstCores * 100).toLocaleString() }));
          }
          plusPerks.push(t(`${page}.perks.fasterFiles`, {}));
          if (economy.neoPlusPercent !== null && economy.neoPlusPercent > 0) {
            plusPerks.push(t(`${page}.perks.neoPlus`, { percent: economy.neoPlusPercent.toLocaleString() }));
          }
          if (economy.devbox) {
            plusPerks.push(size(`${page}.perks.devbox`, economy.devbox.standard));
          }
          if (economy.panelAccess) {
            plusPerks.push(t(`${page}.perks.panelAccess`, {}));
          }

          const plusplusPerks = [
            t(`${page}.perks.premiumLocations`, { plus: economy.plusName }),
            t(`${page}.perks.highestPriority`, {}),
            t(`${page}.perks.noAds`, {}),
          ];
          if (economy.plusplusBurstCores > 0) {
            plusplusPerks.push(t(`${page}.perks.burst`, { cpu: (economy.plusplusBurstCores * 100).toLocaleString() }));
          }
          plusplusPerks.push(t(`${page}.perks.fastestFiles`, {}));
          if (economy.neoPlusplusPercent !== null && economy.neoPlusplusPercent > 0) {
            plusplusPerks.push(
              t(`${page}.perks.neoPlusplus`, { percent: economy.neoPlusplusPercent.toLocaleString() }),
            );
          }
          if (economy.devbox) {
            plusplusPerks.push(size(`${page}.perks.devboxPlusplus`, economy.devbox.premium));
          }
          if (economy.panelAccess) {
            plusplusPerks.push(t(`${page}.perks.panelAccess`, {}));
          }
          // plugged.host: the membership bundle (shared/src/membership_bundle.rs)
          if (economy.bundleName) {
            plusplusPerks.push(t(`${page}.perks.bundle`, { name: economy.bundleName }));
          }

          const faq: { question: string; answer: string }[] = [
            {
              question: t(`${page}.faq.renew.question`, {}),
              answer: economy.autoRenewEnabled
                ? t(`${page}.faq.renew.answerAutoRenew`, { plus: economy.plusName, plusplus: economy.plusplusName })
                : t(`${page}.faq.renew.answer`, { plus: economy.plusName, plusplus: economy.plusplusName }),
            },
            { question: t(`${page}.faq.expiry.question`, {}), answer: t(`${page}.faq.expiry.answer`, {}) },
          ];
          if (economy.idleStop) {
            faq.push({
              question: t(`${page}.faq.idle.question`, {}),
              answer: economy.idleDelete ? t(`${page}.faq.idle.answerDelete`, {}) : t(`${page}.faq.idle.answer`, {}),
            });
          }
          faq.push({ question: t(`${page}.faq.tokens.question`, {}), answer: t(`${page}.faq.tokens.answer`, {}) });
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

          const earnMore = (
            <p className='theme-eco-small'>
              {t(`${page}.balance`, { balance: economy.tokens.toLocaleString() })}{' '}
              <Link to={to('/tokens', '/account')} className='theme-eco-link'>
                {t(`${page}.earnLink`, {})}
              </Link>
            </p>
          );

          const costText = (pay: Offer['pay'], cost: number) =>
            t(`${page}.cost.${pay}`, { cost: cost.toLocaleString() });
          // An offer's button buys it; while the balance is short it says how much is missing and
          // leads to where it is earned instead of sitting there disabled.
          const offerButton = (
            key: string,
            offer: { pay: Offer['pay']; cost: number; fullCost: number; days: number },
            filled: boolean,
            onBuy: () => void,
            disabled = false,
          ) => {
            const missing = offer.cost - balanceOf(offer.pay);

            return (
              <Button
                key={key}
                className='theme-plus-offer w-full'
                variant={filled ? 'filled' : 'default'}
                disabled={disabled}
                onClick={
                  missing > 0
                    ? () => navigate(offer.pay === 'tokens' ? to('/tokens', '/account') : to('/store', '/account'))
                    : onBuy
                }
              >
                <span className='font-semibold'>{t(`${page}.days`, { days: offer.days })}</span>
                <span className='theme-plus-offer-cost'>
                  {missing > 0 ? (
                    t(`${page}.moreToGo`, { amount: tItem(offer.pay === 'tokens' ? 'token' : 'credit', missing) })
                  ) : (
                    <>
                      {offer.fullCost > offer.cost && (
                        <s className='theme-eco-was' aria-hidden='true'>
                          {offer.fullCost.toLocaleString()}
                        </s>
                      )}
                      {costText(offer.pay, offer.cost)}
                    </>
                  )}
                </span>
              </Button>
            );
          };

          // a membership that runs may renew by itself: tokens first, the second with credits when
          // only credits buy it (shared/src/economy.rs renewal_pay)
          const plusplusRenewal = tokensOffer ?? creditsOffer;
          const autoRenewSwitch = (kind: MembershipKind, name: string, checked: boolean, cost: string) => (
            <Switch
              label={t(`${page}.autoRenew.label`, {})}
              description={t(`${page}.autoRenew.description`, { cost })}
              checked={checked}
              disabled={switching}
              onChange={(event) => toggleAutoRenew(kind, name, event.currentTarget.checked)}
            />
          );

          // gifts: either membership, with tokens, at its (sale) price
          const giftOffers = [
            ...(economy.tokensEnabled
              ? [{ kind: 'plus' as const, name: economy.plusName, cost: economy.plusCost, days: economy.plusDays }]
              : []),
            ...(tokensOffer
              ? [
                  {
                    kind: 'plusplus' as const,
                    name: economy.plusplusName,
                    cost: tokensOffer.cost,
                    days: tokensOffer.days,
                  },
                ]
              : []),
          ];
          const giftOffer = giftOffers.find((offer) => offer.kind === giftKind) ?? giftOffers[0];
          const giftMissing = giftOffer ? giftOffer.cost - economy.tokens : 0;
          const sendGift = (event: FormEvent) => {
            event.preventDefault();
            if (!giftOffer || !giftTo.trim()) return;

            setGifting({
              recipient: giftTo.trim(),
              kind: giftOffer.kind,
              name: giftOffer.name,
              price: giftOffer.cost,
              days: giftOffer.days,
            });
          };

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
                    days: economy.plusDays,
                    cost: buying.price.toLocaleString(),
                  }).md()}
                {buying?.kind === 'plusplus' &&
                  t(`${page}.modal.plusplus.${buying.offer.pay}`, {
                    name: economy.plusplusName,
                    days: buying.offer.days,
                    cost: buying.offer.cost.toLocaleString(),
                  }).md()}
              </ConfirmationModal>

              <ConfirmationModal
                opened={!!gifting}
                onClose={() => setGifting(null)}
                title={t(`${page}.gift.modal.title`, {})}
                confirm={t(`${page}.gift.modal.confirm`, {})}
                confirmColor='blue'
                onConfirmed={doGift}
              >
                {gifting &&
                  t(`${page}.gift.modal.body`, {
                    name: gifting.name,
                    days: gifting.days,
                    cost: gifting.price.toLocaleString(),
                    recipient: gifting.recipient,
                  }).md()}
              </ConfirmationModal>

              {!economy.membershipEnabled ? (
                <p className='theme-eco-muted'>{t(`${page}.unavailable`, { name: economy.plusName })}</p>
              ) : (
                <>
                  {economy.discount && (
                    <Alert color='blue' title={economy.discount.name ?? t(`${page}.sale.defaultName`, {})}>
                      {economy.discount.ends
                        ? t(`${page}.sale.until`, {
                            percent: economy.discount.percent,
                            date: new Intl.DateTimeFormat(intlLocale(language), {
                              dateStyle: 'medium',
                              timeStyle: 'short',
                            }).format(economy.discount.ends),
                          })
                        : t(`${page}.sale.running`, { percent: economy.discount.percent })}
                    </Alert>
                  )}

                  {activeUntil && (
                    <Notification
                      className='theme-callout'
                      color='green'
                      title={t(`${page}.active.heading`, {
                        name: hasPlusplus ? economy.plusplusName : economy.plusName,
                      })}
                    >
                      {t(`${page}.active.body`, { until: formatDateTime(activeUntil) })}
                    </Notification>
                  )}

                  {/* plugged.host: Plus with the other panel's second membership (membership_bundle.rs) */}
                  {economy.bundlePlusUntil && !hasPlusplus && (
                    <Notification
                      className='theme-callout'
                      color='green'
                      title={t(`${page}.bundleActive.heading`, {
                        plus: economy.plusName,
                        name: economy.bundleSourceName,
                      })}
                    >
                      {t(`${page}.active.body`, { until: formatDateTime(economy.bundlePlusUntil) })}
                    </Notification>
                  )}

                  <div className='theme-plus-plans'>
                    <PlanCard
                      name={t(`${page}.pageTitle`, { app: appName, name: economy.plusName })}
                      pitch={t(`${page}.plus.pitch`, {})}
                      price={costText('tokens', economy.plusCost)}
                      was={economy.plusFullCost > economy.plusCost ? economy.plusFullCost.toLocaleString() : undefined}
                      term={duration(economy.plusDays)}
                      perks={plusPerks}
                    >
                      {offerButton(
                        'tokens',
                        {
                          pay: 'tokens',
                          cost: economy.plusCost,
                          fullCost: economy.plusFullCost,
                          days: economy.plusDays,
                        },
                        plusFilled,
                        () => setBuying({ kind: 'plus', price: economy.plusCost }),
                        !economy.tokensEnabled,
                      )}

                      {plusShort ? earnMore : <p className='theme-eco-small'>{t(`${page}.extendHint`, {})}</p>}

                      {economy.autoRenewEnabled &&
                        hasPlus &&
                        autoRenewSwitch(
                          'plus',
                          economy.plusName,
                          economy.plusAutoRenew,
                          costText('tokens', economy.plusCost),
                        )}
                    </PlanCard>

                    <PlanCard
                      name={t(`${page}.pageTitle`, { app: appName, name: economy.plusplusName })}
                      pitch={t(`${page}.plusplus.pitch`, {})}
                      price={
                        pricedOffer ? costText(pricedOffer.pay, pricedOffer.cost) : t(`${page}.plusplus.notSold`, {})
                      }
                      was={
                        pricedOffer && pricedOffer.fullCost > pricedOffer.cost
                          ? pricedOffer.fullCost.toLocaleString()
                          : undefined
                      }
                      term={plusplusTerm}
                      perks={plusplusPerks}
                    >
                      {economy.plusplusOffers.map((offer) =>
                        offerButton(offer.pay, offer, offer.pay !== 'credits' && !plusFilled, () =>
                          setBuying({ kind: 'plusplus', offer }),
                        ),
                      )}

                      {economy.plusplusOffers.length === 0 ? (
                        <p className='theme-eco-small'>{t(`${page}.plusplus.notSold`, {})}</p>
                      ) : plusplusShort ? (
                        earnMore
                      ) : (
                        <p className='theme-eco-small'>{t(`${page}.extendHint`, {})}</p>
                      )}

                      {economy.autoRenewEnabled &&
                        hasPlusplus &&
                        plusplusRenewal &&
                        autoRenewSwitch(
                          'plusplus',
                          economy.plusplusName,
                          economy.plusplusAutoRenew,
                          costText(plusplusRenewal.pay, plusplusRenewal.cost),
                        )}
                    </PlanCard>
                  </div>

                  {economy.giftsEnabled && giftOffer && (
                    <section className='flex flex-col gap-3'>
                      <div className='flex flex-col gap-1'>
                        <h3 className='theme-eco-heading'>{t(`${page}.gift.heading`, {})}</h3>
                        <p className='theme-eco-muted'>{t(`${page}.gift.text`, {})}</p>
                      </div>

                      <form className='flex flex-col gap-3' onSubmit={sendGift}>
                        <div className='grid gap-3 sm:grid-cols-2'>
                          <TextInput
                            label={t(`${page}.gift.recipient`, {})}
                            value={giftTo}
                            maxLength={255}
                            autoComplete='off'
                            onChange={(event) => setGiftTo(event.currentTarget.value)}
                          />
                          <Select
                            label={t(`${page}.gift.membership`, {})}
                            data={giftOffers.map((offer) => ({
                              value: offer.kind,
                              label: t(`${page}.gift.option`, {
                                name: offer.name,
                                days: offer.days,
                                cost: costText('tokens', offer.cost),
                              }),
                            }))}
                            value={giftOffer.kind}
                            onChange={(value) => {
                              const kind = membershipKindSchema.safeParse(value);
                              if (kind.success) setGiftKind(kind.data);
                            }}
                          />
                        </div>

                        <div className='flex flex-col items-start gap-2'>
                          <Button
                            type='submit'
                            variant='default'
                            disabled={!giftTo.trim() || economy.giftsLeft <= 0 || giftMissing > 0}
                          >
                            {t(`${page}.gift.submit`, {})}
                          </Button>

                          <p className='theme-eco-small'>
                            {economy.giftsLeft <= 0 ? (
                              t(`${page}.gift.none`, {})
                            ) : giftMissing > 0 ? (
                              <Link to={to('/tokens', '/account')} className='theme-eco-link'>
                                {t(`${page}.moreToGo`, { amount: tItem('token', giftMissing) })}
                              </Link>
                            ) : (
                              t(`${page}.gift.left`, { count: economy.giftsLeft })
                            )}
                          </p>
                        </div>
                      </form>
                    </section>
                  )}

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
