// plugged.host: the Resource Store, laid out like the Pelican panel's: the upgrades not claimed
// yet, the credit balance, one row per resource with its meter and offers, then the credit
// history. No filled button: every purchase is a gray one with its price (docs/features/economy.md).
import { Button as MantineButton } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import classNames from 'classnames';
import { ReactNode, useState } from 'react';
import { z } from 'zod';
import { getHttpStatus, httpErrorToHuman } from '@/api/axios.ts';
import getStore from '@/api/me/economy/getStore.ts';
import purchaseResource from '@/api/me/economy/purchaseResource.ts';
import { AdPageSlot } from '@/elements/ads/AdPlacements.tsx';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import TransactionRows from '@/elements/economy/TransactionRows.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { storeCatalogEntrySchema, storeResourceSchema, userStoreSchema } from '@/lib/schemas/user/economy.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import StorePerks from './StorePerks.tsx';

type Resource = z.infer<typeof storeResourceSchema>;
type Entry = z.infer<typeof storeCatalogEntrySchema>;
type Offer = Entry['offers'][number];
type Usage = z.infer<typeof userStoreSchema>['usage'][number];

const page = 'pages.account.store';
const resourcesId = 'store-resources';

// a price this high stands for "more than anyone can pay" (shared/src/pricing.rs)
const UNAFFORDABLE = 1_000_000_000;

/** "50%", "4 GiB", "1.5 GiB", "512 MiB": CPU in percent, memory and disk from MiB. */
function amountOf(resource: Resource, value: number): string {
  return resource === 'cpu' ? `${value}%` : bytesToString(mbToBytes(value), 1);
}

/**
 * Widths in % of the track, of what is used and of the limit behind it. The track runs to the soft
 * limit, where prices get steep (further when the account is past it); a limit of 0 is none.
 */
function meter(used: number, limit: number, soft: number) {
  const track = Math.max(soft, limit, used, 1);
  const percent = (value: number) => Math.round((Math.min(Math.max(value, 0), track) / track) * 1000) / 10;

  return { used: percent(used), limit: percent(limit), over: limit > 0 && used > limit };
}

/** A toast with a heading, as the Pelican panel's notifications have. */
function ToastBody({ title, children }: { title: string; children: ReactNode }) {
  return (
    <>
      <span className='block font-semibold'>{title}</span>
      {children}
    </>
  );
}

function ResourceRow({
  usage,
  entry,
  credits,
  onBuy,
}: {
  usage: Usage;
  /** What is for sale, when anything is. */
  entry: Entry | null;
  credits: number;
  onBuy: (entry: Entry, offer: Offer) => void;
}) {
  const { t, language } = useTranslations();
  const number = new Intl.NumberFormat(language);

  const offers = entry?.offers ?? [];
  const bar = meter(usage.used, usage.limit, entry?.softLimit ?? 0);
  const used = amountOf(usage.resource, usage.used);
  const limit = amountOf(usage.resource, usage.limit);
  const over = usage.limit > 0 && usage.used > usage.limit ? amountOf(usage.resource, usage.used - usage.limit) : null;

  // what the next steps cost, read off what the account has against where prices change
  const priceNote =
    entry && offers.length > 0 && entry.softLimit > 0
      ? entry.total >= entry.softLimit
        ? t(`${page}.price.steep`, { soft: amountOf(entry.resource, entry.softLimit) })
        : entry.total >= entry.limit
          ? t(`${page}.price.climbing`, {
              limit: amountOf(entry.resource, entry.limit),
              soft: amountOf(entry.resource, entry.softLimit),
            })
          : t(`${page}.price.rising`, { limit: amountOf(entry.resource, entry.limit) })
      : null;

  return (
    <div className='theme-eco-row'>
      <div className='flex min-w-0 flex-1 flex-col gap-2'>
        <div className='flex items-baseline justify-between gap-3'>
          <h4 className='theme-eco-title'>{t(`${page}.resource.${usage.resource}`, {})}</h4>
          <span className='theme-eco-small tabular-nums'>
            <span className={classNames('theme-eco-num', over !== null && 'theme-eco-danger')}>{used}</span>
            {usage.limit > 0 && ` ${t(`${page}.ofLimit`, { limit })}`}
          </span>
        </div>

        <div
          className='theme-eco-meter'
          role='img'
          aria-label={usage.limit > 0 ? t(`${page}.meterLabel`, { used, limit }) : t(`${page}.inUse`, { used })}
        >
          <span className='theme-eco-meter-limit' style={{ width: `${bar.limit}%` }} />
          <span
            className={bar.over ? 'theme-eco-meter-over' : 'theme-eco-meter-used'}
            style={{ width: `${bar.used}%` }}
          />
        </div>

        {(over !== null || priceNote !== null) && (
          <p className='theme-eco-small'>
            {over !== null ? (
              <span className='theme-eco-danger'>{t(`${page}.over`, { amount: over })}</span>
            ) : (
              priceNote
            )}
          </p>
        )}
      </div>

      {entry && offers.length > 0 && (
        <div className='theme-eco-row-action'>
          {offers.map((offer) => (
            <Button
              key={offer.steps}
              variant='default'
              className='w-full'
              disabled={offer.price >= UNAFFORDABLE || offer.price > credits}
              onClick={() => onBuy(entry, offer)}
            >
              <span>+{amountOf(entry.resource, offer.amount)}</span>
              <span className='tabular-nums'>
                {number.format(offer.price)} {t(`${page}.creditsShort`, {})}
              </span>
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}

export default function DashboardStore() {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const number = new Intl.NumberFormat(language);

  const store = useResource({
    queryKey: queryKeys.user.store(),
    queryFn: getStore,
  });

  const [buying, setBuying] = useState<{ entry: Entry; offer: Offer } | null>(null);

  const doBuy = async () => {
    if (!buying) return;
    const { entry, offer } = buying;

    try {
      const updated = await purchaseResource(entry.resource, offer.steps, offer.price);
      queryClient.setQueryData(queryKeys.user.store(), updated);
      queryClient.invalidateQueries({ queryKey: queryKeys.user.membership() });
      queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
      queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.createOptions() });
      addToast(
        <ToastBody title={t(`${page}.toast.successful`, {})}>
          {t(`${page}.toast.bought.${entry.resource}`, {
            amount: amountOf(entry.resource, offer.amount),
            cost: number.format(offer.price),
          })}
        </ToastBody>,
        'success',
      );
    } catch (error) {
      // the price moved since: show the new ones
      if (getHttpStatus(error) === 409) {
        store.invalidate();
      }

      addToast(<ToastBody title={t(`${page}.toast.failed`, {})}>{httpErrorToHuman(error)}</ToastBody>, 'error');
    } finally {
      setBuying(null);
    }
  };

  const openResources = () =>
    document.getElementById(resourcesId)?.scrollIntoView({ behavior: 'smooth', block: 'start' });

  return (
    <AccountContentContainer title={t(`${page}.title`, {})} subtitle={t(`${page}.subheading`, {})}>
      <ConfirmationModal
        opened={!!buying}
        onClose={() => setBuying(null)}
        title={t(`${page}.modal.title`, {})}
        confirm={t(`${page}.modal.confirm`, {})}
        confirmColor='blue'
        onConfirmed={doBuy}
      >
        {buying &&
          t(`${page}.modal.content`, {
            item: t(`${page}.modal.item`, {
              amount: amountOf(buying.entry.resource, buying.offer.amount),
              resource: t(`${page}.resource.${buying.entry.resource}`, {}),
            }),
            cost: number.format(buying.offer.price),
          })}
      </ConfirmationModal>

      <ResourceView resource={store}>
        {(store) => (
          <div className='flex flex-col gap-6'>
            <StorePerks perks={store.perks} credits={store.credits} onOpenStore={openResources} />

            <section className='theme-eco-card'>
              <div className='flex flex-col gap-1'>
                <span className='theme-eco-heading'>{t(`${page}.credits`, {})}</span>
                <span className='theme-eco-value'>{number.format(store.credits)}</span>
              </div>

              <div className='flex flex-col gap-1'>
                <p className='theme-eco-text'>{t(`${page}.whatCreditsBuy`, {})}</p>
                {store.rewardsUrl && <p className='theme-eco-muted'>{t(`${page}.balanceHint`, {})}</p>}
              </div>

              {store.rewardsUrl && (
                <div>
                  <MantineButton
                    component='a'
                    href={store.rewardsUrl}
                    target='_blank'
                    rel='noopener noreferrer'
                    variant='default'
                    fw={600}
                  >
                    {t(`${page}.earnDiscordRewards`, {})}
                  </MantineButton>
                </div>
              )}
            </section>

            {store.hasPlan && store.usage.length > 0 && (
              <section id={resourcesId} className='flex flex-col gap-4'>
                <div className='flex flex-col gap-1'>
                  <h3 className='theme-eco-heading'>{t(`${page}.resourcesHeading`, {})}</h3>
                  <p className='theme-eco-muted max-w-2xl'>{t(`${page}.resourcesHint`, {})}</p>
                </div>

                <div className='theme-eco-list'>
                  {store.usage.map((usage) => (
                    <ResourceRow
                      key={usage.resource}
                      usage={usage}
                      entry={
                        store.available
                          ? (store.catalog.find((entry) => entry.resource === usage.resource) ?? null)
                          : null
                      }
                      credits={store.credits}
                      onBuy={(entry, offer) => setBuying({ entry, offer })}
                    />
                  ))}
                </div>
              </section>
            )}

            <section className='flex flex-col gap-3'>
              <h3 className='theme-eco-heading'>{t(`${page}.history`, {})}</h3>

              <div className='theme-eco-list'>
                {store.transactions.length === 0 ? (
                  <p className='theme-eco-muted theme-eco-empty'>{t(`${page}.noTransactions`, {})}</p>
                ) : (
                  <TransactionRows transactions={store.transactions} variant='ledger' />
                )}
              </div>
            </section>
          </div>
        )}
      </ResourceView>
      <AdPageSlot />
    </AccountContentContainer>
  );
}
