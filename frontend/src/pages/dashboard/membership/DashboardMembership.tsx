// plugged.host: the memberships: the first bought once with tokens, the second for a while with
// tokens or credits (docs/features/economy.md).
import { useQueryClient } from '@tanstack/react-query';
import { ReactNode, useState } from 'react';
import { z } from 'zod';
import buyPlus from '@/api/me/economy/buyPlus.ts';
import buyPlusPlus from '@/api/me/economy/buyPlusPlus.ts';
import getMembership from '@/api/me/economy/getMembership.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { userEconomySchema } from '@/lib/schemas/user/economy.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Economy = z.infer<typeof userEconomySchema>;
type Offer = Economy['plusplusOffers'][number];

const page = 'pages.account.membership';

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

export default function DashboardMembership() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();

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

  const balance = (economy: Economy, pay: Offer['pay']) => (pay === 'tokens' ? economy.tokens : economy.credits);

  return (
    <AccountContentContainer title={t(`${page}.title`, {})}>
      <ResourceView resource={membership}>
        {(economy) => (
          <Stack gap='xl'>
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

            <Text>
              {t(`${page}.balance`, {
                tokens: economy.tokens.toLocaleString(),
                credits: economy.credits.toLocaleString(),
              })}
            </Text>

            {!economy.membershipEnabled ? (
              <Text size='sm' c='dimmed'>
                {t(`${page}.disabled`, {})}
              </Text>
            ) : (
              <>
                <Section title={economy.plusName}>
                  <Text size='sm' c='dimmed'>
                    {t(`${page}.plus.description`, { name: economy.plusName })}
                  </Text>
                  {economy.plusBurstCores > 0 && (
                    <Text size='sm' c='dimmed'>
                      {t(`${page}.burst`, { cpu: economy.plusBurstCores * 100 })}
                    </Text>
                  )}
                  {economy.plusSince ? (
                    <Text size='sm'>
                      {t(`${page}.plus.since`, { name: economy.plusName })}{' '}
                      <FormattedTimestamp timestamp={economy.plusSince} />
                    </Text>
                  ) : (
                    <Group>
                      <Button
                        disabled={!economy.tokensEnabled || economy.tokens < economy.plusCost}
                        onClick={() => setBuying({ kind: 'plus' })}
                      >
                        {t(`${page}.plus.buy`, { name: economy.plusName, cost: economy.plusCost.toLocaleString() })}
                      </Button>
                    </Group>
                  )}
                </Section>

                <Section title={economy.plusplusName}>
                  <Text size='sm' c='dimmed'>
                    {t(`${page}.plusplus.description`, { name: economy.plusplusName, plus: economy.plusName })}
                  </Text>
                  {economy.plusplusBurstCores > 0 && (
                    <Text size='sm' c='dimmed'>
                      {t(`${page}.burst`, { cpu: economy.plusplusBurstCores * 100 })}
                    </Text>
                  )}
                  {economy.plusplusUntil && (
                    <Text size='sm'>
                      {t(`${page}.plusplus.until`, { name: economy.plusplusName })}{' '}
                      <FormattedTimestamp timestamp={economy.plusplusUntil} />
                    </Text>
                  )}
                  {!economy.plusSince ? (
                    <Text size='sm' c='dimmed'>
                      {t(`${page}.plusplus.needsPlus`, { plus: economy.plusName })}
                    </Text>
                  ) : economy.plusplusOffers.length === 0 ? (
                    <Text size='sm' c='dimmed'>
                      {t(`${page}.plusplus.notSold`, {})}
                    </Text>
                  ) : (
                    <Group gap='xs'>
                      {economy.plusplusOffers.map((offer, index) => (
                        <Button
                          key={offer.pay}
                          variant={index === 0 ? 'filled' : 'default'}
                          disabled={balance(economy, offer.pay) < offer.cost}
                          onClick={() => setBuying({ kind: 'plusplus', offer })}
                        >
                          {t(`${page}.plusplus.buy.${offer.pay}`, {
                            days: offer.days,
                            cost: offer.cost.toLocaleString(),
                          })}
                        </Button>
                      ))}
                    </Group>
                  )}
                </Section>
              </>
            )}
          </Stack>
        )}
      </ResourceView>
    </AccountContentContainer>
  );
}
