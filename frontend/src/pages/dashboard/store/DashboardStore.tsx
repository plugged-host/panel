// plugged.host: the Resource Store: credits buy CPU, memory and disk for good
// (docs/features/economy.md).
import { useQueryClient } from '@tanstack/react-query';
import { ReactNode, useState } from 'react';
import { z } from 'zod';
import { getHttpStatus } from '@/api/axios.ts';
import getStore from '@/api/me/economy/getStore.ts';
import purchaseResource from '@/api/me/economy/purchaseResource.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import TransactionsTable from '@/elements/economy/TransactionsTable.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { storeCatalogEntrySchema, storeResourceSchema } from '@/lib/schemas/user/economy.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Entry = z.infer<typeof storeCatalogEntrySchema>;
type Offer = Entry['offers'][number];

// a price this high stands for "more than anyone can pay" (shared/src/pricing.rs)
const UNAFFORDABLE = 1_000_000_000;

function amountOf(resource: z.infer<typeof storeResourceSchema>, value: number): string {
  return resource === 'cpu' ? `${value}%` : bytesToString(mbToBytes(value));
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Stack gap='xs'>
      <Title order={3} fz='md' fw={600}>
        {title}
      </Title>
      {children}
    </Stack>
  );
}

export default function DashboardStore() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const page = 'pages.account.store';

  const store = useResource({
    queryKey: queryKeys.user.store(),
    queryFn: getStore,
  });

  const [buying, setBuying] = useState<{ entry: Entry; offer: Offer } | null>(null);

  const doBuy = async () => {
    if (!buying) return;

    try {
      const updated = await purchaseResource(buying.entry.resource, buying.offer.steps, buying.offer.price);
      queryClient.setQueryData(queryKeys.user.store(), updated);
      queryClient.invalidateQueries({ queryKey: queryKeys.user.membership() });
      queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.createOptions() });
      addToast(
        t(`${page}.toast.bought`, {
          amount: amountOf(buying.entry.resource, buying.offer.amount),
          resource: t(`${page}.resource.${buying.entry.resource}`, {}),
        }),
        'success',
      );
      setBuying(null);
    } catch (error) {
      if (getHttpStatus(error) === 409) {
        // the price moved: show the new ones
        store.invalidate();
        setBuying(null);
      }

      throw error;
    }
  };

  return (
    <AccountContentContainer title={t(`${page}.title`, {})}>
      <Text size='sm' c='dimmed' mb='md'>
        {t(`${page}.description`, {})}
      </Text>

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
            amount: amountOf(buying.entry.resource, buying.offer.amount),
            resource: t(`${page}.resource.${buying.entry.resource}`, {}),
            price: buying.offer.price.toLocaleString(),
          }).md()}
      </ConfirmationModal>

      <ResourceView resource={store}>
        {(store) => (
          <Stack gap='xl'>
            <Text>{t(`${page}.balance`, { credits: store.credits.toLocaleString() })}</Text>

            {!store.available ? (
              <Text size='sm' c='dimmed'>
                {t(`${page}.unavailable`, {})}
              </Text>
            ) : (
              store.catalog.map((entry) => (
                <Section key={entry.resource} title={t(`${page}.resource.${entry.resource}`, {})}>
                  <Text size='sm' c='dimmed'>
                    {t(`${page}.has`, { amount: amountOf(entry.resource, entry.total) })}{' '}
                    {entry.total >= entry.softLimit
                      ? t(`${page}.price.steep`, { soft: amountOf(entry.resource, entry.softLimit) })
                      : entry.total >= entry.limit
                        ? t(`${page}.price.climbing`, {
                            limit: amountOf(entry.resource, entry.limit),
                            soft: amountOf(entry.resource, entry.softLimit),
                          })
                        : t(`${page}.price.rising`, { limit: amountOf(entry.resource, entry.limit) })}
                  </Text>
                  {entry.offers.length === 0 ? (
                    <Text size='sm' c='dimmed'>
                      {t(`${page}.notSold`, {})}
                    </Text>
                  ) : (
                    <Group gap='xs'>
                      {entry.offers.map((offer) => (
                        <Button
                          key={offer.steps}
                          variant='default'
                          disabled={offer.price >= UNAFFORDABLE || offer.price > store.credits}
                          onClick={() => setBuying({ entry, offer })}
                        >
                          {t(`${page}.offer`, {
                            amount: amountOf(entry.resource, offer.amount),
                            price: offer.price.toLocaleString(),
                          })}
                        </Button>
                      ))}
                    </Group>
                  )}
                </Section>
              ))
            )}

            <Section title={t(`${page}.history`, {})}>
              <TransactionsTable transactions={store.transactions} />
            </Section>
          </Stack>
        )}
      </ResourceView>
    </AccountContentContainer>
  );
}
