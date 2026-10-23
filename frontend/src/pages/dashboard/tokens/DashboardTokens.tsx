// plugged.host: tokens: the balance, earning them through sponsored links, and the history
// (docs/features/economy.md).
import { Button as MantineButton } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { ReactNode, useEffect, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getTokens from '@/api/me/economy/getTokens.ts';
import redeemTokens from '@/api/me/economy/redeemTokens.ts';
import startEarn from '@/api/me/economy/startEarn.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import TransactionsTable from '@/elements/economy/TransactionsTable.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { armFullScript } from '@/lib/linkvertise.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { redemptionSchema, userTokensSchema } from '@/lib/schemas/user/economy.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Provider = z.infer<typeof userTokensSchema>['providers'][number];

const page = 'pages.account.tokens';

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

// Linkvertise's Full Script: the link is on the page from the start (the claim is opened when the
// page loads) and the script turns it into a Linkvertise link.
function FullScriptLink({ provider, capped }: { provider: Provider; capped: boolean }) {
  const { t } = useTranslations();
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (capped) return;

    startEarn(provider.provider)
      .then((earn) => setUrl(earn.url))
      .catch((error) => setError(httpErrorToHuman(error)));
  }, [provider.provider, capped]);

  useEffect(() => {
    if (!url || provider.publisher === null) return;

    armFullScript(provider.publisher, provider.whitelist).catch(() => setError(t(`${page}.scriptBlocked`, {})));
  }, [url]);

  if (error) {
    return (
      <Text size='sm' c='red'>
        {error}
      </Text>
    );
  }

  return url && !capped ? (
    // a real link: the script only rewrites anchors
    <MantineButton component='a' href={url} variant='default' fw={600}>
      {t(`${page}.earn`, { reward: provider.reward })}
    </MantineButton>
  ) : (
    <Button variant='default' disabled>
      {t(`${page}.earn`, { reward: provider.reward })}
    </Button>
  );
}

function EarnButton({ provider, capped }: { provider: Provider; capped: boolean }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const [loading, setLoading] = useState(false);

  const go = () => {
    setLoading(true);
    startEarn(provider.provider)
      .then((earn) => {
        window.location.href = earn.url;
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
        setLoading(false);
      });
  };

  return (
    <Button variant='default' disabled={capped} loading={loading} onClick={go}>
      {t(`${page}.earn`, { reward: provider.reward })}
    </Button>
  );
}

export default function DashboardTokens() {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  const [redemption, setRedemption] = useState<z.infer<typeof redemptionSchema> | null>(null);
  const [redeemError, setRedeemError] = useState<string | null>(null);
  const [redeeming, setRedeeming] = useState(() => searchParams.has('claim'));
  const redeemed = useRef(false);

  const tokens = useResource({
    queryKey: queryKeys.user.tokens(),
    queryFn: getTokens,
    enabled: !redeeming,
  });

  // back from a sponsored link: settle the claim once, then drop the parameters from the address
  useEffect(() => {
    const claim = searchParams.get('claim');
    if (!claim || redeemed.current) return;
    redeemed.current = true;

    const params: Record<string, string> = {};
    searchParams.forEach((value, key) => {
      if (key !== 'claim') params[key] = value;
    });

    redeemTokens(claim, params)
      .then((result) => {
        setRedemption(result);
        queryClient.invalidateQueries({ queryKey: queryKeys.user.membership() });
      })
      .catch((error) => setRedeemError(httpErrorToHuman(error)))
      .finally(() => {
        setRedeeming(false);
        navigate('/account/tokens', { replace: true });
        queryClient.invalidateQueries({ queryKey: queryKeys.user.tokens() });
      });
  }, []);

  return (
    <AccountContentContainer title={t(`${page}.title`, {})}>
      <Text size='sm' c='dimmed' mb='md'>
        {t(`${page}.description`, {})}
      </Text>

      {redeemError && (
        <Alert color='red' mb='md' withCloseButton onClose={() => setRedeemError(null)}>
          {redeemError}
        </Alert>
      )}
      {redemption && (
        <Alert
          color={redemption.status === 'granted' || redemption.status === 'settled' ? 'green' : 'yellow'}
          mb='md'
          withCloseButton
          onClose={() => setRedemption(null)}
        >
          {redemption.status === 'granted' || redemption.status === 'settled'
            ? t(`${page}.redemption.${redemption.status}`, { amount: redemption.amount })
            : t(`${page}.redemption.${redemption.status}`, {})}
        </Alert>
      )}

      <ResourceView resource={tokens}>
        {(tokens) => (
          <Stack gap='xl'>
            <Text>
              {t(`${page}.balance`, {
                tokens: tokens.tokens.toLocaleString(),
                earned: tokens.tokensEarned.toLocaleString(),
              })}
            </Text>

            {!tokens.enabled ? (
              <Text size='sm' c='dimmed'>
                {t(`${page}.disabled`, {})}
              </Text>
            ) : (
              <Section title={t(`${page}.earnTitle`, {})}>
                {tokens.providers.length === 0 ? (
                  <Text size='sm' c='dimmed'>
                    {t(`${page}.noProviders`, {})}
                  </Text>
                ) : (
                  <Stack gap='md'>
                    <Text size='sm' c='dimmed'>
                      {t(`${page}.earnHint`, { minutes: tokens.claimMinutes })}
                    </Text>
                    {tokens.providers.map((provider) => {
                      const capped = provider.dailyCap > 0 && provider.grantedToday >= provider.dailyCap;

                      return (
                        <Group key={provider.provider} justify='space-between' wrap='wrap'>
                          <Stack gap={0}>
                            <Text fw={600}>{t(`${page}.provider.${provider.provider}`, {})}</Text>
                            <Text size='sm' c='dimmed'>
                              {provider.dailyCap > 0
                                ? t(`${page}.today`, { granted: provider.grantedToday, cap: provider.dailyCap })
                                : t(`${page}.noCap`, {})}
                            </Text>
                          </Stack>
                          {provider.fullScript ? (
                            <FullScriptLink provider={provider} capped={capped} />
                          ) : (
                            <EarnButton provider={provider} capped={capped} />
                          )}
                        </Group>
                      );
                    })}
                  </Stack>
                )}
              </Section>
            )}

            <Section title={t(`${page}.history`, {})}>
              <TransactionsTable transactions={tokens.transactions} />
            </Section>
          </Stack>
        )}
      </ResourceView>
    </AccountContentContainer>
  );
}
