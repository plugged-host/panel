// plugged.host: tokens, laid out like the Pelican panel's Tokens page: the balance, the ways to earn
// more through sponsored links, and the recent activity. The first way's button is the page's one
// filled button, any further one is gray (docs/features/economy.md).
import { Button as MantineButton } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { ReactNode, useEffect, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { z } from 'zod';
import { getHttpStatus } from '@/api/axios.ts';
import getTokens from '@/api/me/economy/getTokens.ts';
import redeemTokens from '@/api/me/economy/redeemTokens.ts';
import startEarn from '@/api/me/economy/startEarn.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import TransactionRows from '@/elements/economy/TransactionRows.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { intlLocale } from '@/lib/languages.ts';
import { armFullScript } from '@/lib/linkvertise.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { userTokensSchema } from '@/lib/schemas/user/economy.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type Provider = z.infer<typeof userTokensSchema>['providers'][number];

const page = 'pages.account.tokens';

const isCapped = (provider: Provider) => provider.dailyCap > 0 && provider.grantedToday >= provider.dailyCap;

/** Why a link could not be opened, in the words of the return trip's results. */
function earnFailure(error: unknown): 'capped' | 'throttled' | 'error' {
  switch (getHttpStatus(error)) {
    case 409:
      return 'capped';
    case 429:
      return 'throttled';
    default:
      return 'error';
  }
}

// Linkvertise's Full Script: the claim is opened when the page loads and its link is a real anchor
// on the page, which the script turns into a Linkvertise link. `url` is null while it loads.
function useFullScriptLink(provider: Provider, enabled: boolean) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [blocked, setBlocked] = useState(false);

  useEffect(() => {
    if (!enabled) return;

    startEarn(provider.provider)
      .then((earn) => setUrl(earn.url))
      .catch(() => setFailed(true));
  }, [provider.provider, enabled]);

  useEffect(() => {
    if (!url || provider.publisher === null) return;

    armFullScript(provider.publisher, provider.whitelist).catch(() => setBlocked(true));
  }, [url]);

  return { url, failed, blocked };
}

/** A static link: opens a claim and goes to the provider. */
function EarnButton({ provider, filled, fullWidth }: { provider: Provider; filled: boolean; fullWidth?: boolean }) {
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
        addToast(t(`${page}.redemption.${earnFailure(error)}`, {}), 'warning');
        setLoading(false);
      });
  };

  return (
    <Button variant={filled ? 'filled' : 'default'} fullWidth={fullWidth} loading={loading} onClick={go}>
      {t(`${page}.earnWith`, { provider: t(`${page}.provider.${provider.provider}`, {}) })}
    </Button>
  );
}

function EarnRow({ provider, first }: { provider: Provider; first: boolean }) {
  const { t, language } = useTranslations();
  const capped = isCapped(provider);
  const fullScript = useFullScriptLink(provider, provider.fullScript && !capped);
  const label = t(`${page}.provider.${provider.provider}`, {});
  const variant = first ? 'filled' : 'default';

  let action: ReactNode;
  if (capped) {
    action = (
      <Badge color='gray' size='sm'>
        {t(`${page}.capped`, {})}
      </Badge>
    );
  } else if (!provider.fullScript) {
    action = <EarnButton provider={provider} filled={first} fullWidth />;
  } else if (fullScript.failed || fullScript.blocked) {
    action = (
      <Badge color='gray' size='sm'>
        {t(`${page}.linkUnavailable`, {})}
      </Badge>
    );
  } else if (fullScript.url) {
    // a real anchor: the script only rewrites anchors
    action = (
      <MantineButton component='a' href={fullScript.url} variant={variant} fullWidth fw={600}>
        {t(`${page}.earnWith`, { provider: label })}
      </MantineButton>
    );
  } else {
    action = (
      <Button variant={variant} fullWidth loading>
        {t(`${page}.earnWith`, { provider: label })}
      </Button>
    );
  }

  return (
    <div className='theme-earn-row'>
      <div className='flex min-w-0 flex-col gap-0.5'>
        <span className='theme-eco-title'>{label}</span>
        <span className='theme-eco-muted'>
          {t(`${page}.reward`, {
            count: new Intl.NumberFormat(intlLocale(language), { maximumFractionDigits: 2 }).format(
              provider.nextReward,
            ),
          })}
        </span>
        {provider.nextReward < provider.reward && (
          <span className='theme-earn-small'>
            {t(`${page}.decays`, {
              full: new Intl.NumberFormat(intlLocale(language), { maximumFractionDigits: 2 }).format(provider.reward),
            })}
          </span>
        )}
        {provider.dailyCap > 0 && (
          <span className='theme-earn-small'>
            {t(`${page}.used`, { used: provider.grantedToday, cap: provider.dailyCap })}
          </span>
        )}
        {fullScript.blocked && <span className='theme-eco-small'>{t(`${page}.scriptBlocked`, {})}</span>}
      </div>

      <div className='theme-earn-action'>{action}</div>
    </div>
  );
}

export default function DashboardTokens() {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const plusName = useGlobalStore((state) => state.settings.economy?.plusName ?? '');

  const [redeeming, setRedeeming] = useState(() => searchParams.has('claim'));
  const redeemed = useRef(false);

  const tokens = useResource({
    queryKey: queryKeys.user.tokens(),
    queryFn: getTokens,
    enabled: !redeeming,
  });

  // back from a sponsored link: settle the claim once, say how it went, then drop the parameters
  // from the address
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
        if (result.status === 'granted') {
          addToast(t(`${page}.redemption.granted`, { amount: result.amount }), 'success');
        } else {
          addToast(t(`${page}.redemption.${result.status}`, {}), 'warning');
        }

        queryClient.invalidateQueries({ queryKey: queryKeys.user.membership() });
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
      })
      .catch((error) =>
        addToast(t(`${page}.redemption.${getHttpStatus(error) === 429 ? 'throttled' : 'error'}`, {}), 'warning'),
      )
      .finally(() => {
        setRedeeming(false);
        navigate('/account/tokens', { replace: true });
        queryClient.invalidateQueries({ queryKey: queryKeys.user.tokens() });
      });
  }, []);

  // The page header's buttons, one per static link. A Full Script link gets none: it must be the
  // page's only anchor for its claim.
  const headerProviders = tokens.data?.providers.filter((provider) => !provider.fullScript) ?? [];

  return (
    <AccountContentContainer
      title={t(`${page}.title`, {})}
      subtitle={t(`${page}.subheading`, {})}
      contentRight={
        headerProviders.length > 0
          ? headerProviders.map((provider) => <EarnButton key={provider.provider} provider={provider} filled={false} />)
          : undefined
      }
    >
      <ResourceView resource={tokens}>
        {(tokens) => (
          <div className='flex flex-col gap-6'>
            <section className='theme-eco-card'>
              <div className='flex flex-col gap-1'>
                <span className='theme-eco-value'>
                  {t(`${page}.balance`, { count: new Intl.NumberFormat(intlLocale(language)).format(tokens.tokens) })}
                </span>
                <p className='theme-eco-muted'>{t(`${page}.explainer`, { app: appName, plus: plusName })}</p>
                {tokens.tokenFraction > 0 && (
                  <p className='theme-eco-small'>
                    {t(`${page}.saved`, {
                      amount: new Intl.NumberFormat(intlLocale(language), { maximumFractionDigits: 2 }).format(
                        tokens.tokenFraction,
                      ),
                    })}
                  </p>
                )}
              </div>
            </section>

            <section className='theme-eco-card'>
              {tokens.providers.length === 0 ? (
                <p className='theme-eco-muted'>{t(`${page}.none`, {})}</p>
              ) : (
                <div className='theme-earn-rows flex flex-col'>
                  {tokens.providers.map((provider, index) => (
                    <EarnRow key={provider.provider} provider={provider} first={index === 0} />
                  ))}
                </div>
              )}
            </section>

            {tokens.transactions.length > 0 && (
              <TitleCard title={t(`${page}.history`, {})}>
                <div className='theme-earn-rows flex flex-col'>
                  <TransactionRows transactions={tokens.transactions} variant='activity' />
                </div>
              </TitleCard>
            )}
          </div>
        )}
      </ResourceView>
    </AccountContentContainer>
  );
}
