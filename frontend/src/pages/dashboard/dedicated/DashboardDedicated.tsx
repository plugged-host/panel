// plugged.host: the Paid page, apart from the free plans: what Paid adds, the account's Funds, its
// paid servers with their months, and the plans. With noprofit.space connected a plan with a price is
// ordered here and paid with Funds (DedicatedOrderModal.tsx), else ordered from the team with a support
// ticket and set up by an admin (docs/features/dedicated.md)
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { z } from 'zod';
import getBalance from '@/api/me/economy/getBalance.ts';
import getDedicatedPlans from '@/api/server/getDedicatedPlans.ts';
import AddFundsLink, { formatEuro } from '@/elements/AddFundsLink.tsx';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { PaidServer } from '@/lib/schemas/server/billing.ts';
import { serverCreateDedicatedPlanSchema } from '@/lib/schemas/server/create.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import DedicatedOrderModal from './DedicatedOrderModal.tsx';
import { PaidBillingActions, PaidStatus } from './PaidBilling.tsx';

type DedicatedPlan = z.infer<typeof serverCreateDedicatedPlanSchema>;

const page = 'pages.account.dedicated';

/** What Paid adds over the free servers. */
const BENEFITS = ['threads', 'memory', 'queue', 'limbo'] as const;

/** The account's Funds and where to add more; without noprofit.space, that plans go through support. */
function FundsCard({ selfService }: { selfService: boolean }) {
  const { t, language } = useTranslations();
  const balance = useQuery({
    queryKey: queryKeys.user.balance(),
    queryFn: getBalance,
    refetchInterval: 60_000,
    enabled: selfService,
  });

  if (!selfService) {
    return (
      <section className='theme-eco-card'>
        <p className='theme-eco-text'>{t(`${page}.funds.support`, {})}</p>
      </section>
    );
  }

  const funds = balance.data?.funds ?? null;

  return (
    <section className='theme-eco-card'>
      <div className='flex flex-col gap-1'>
        <div className='flex flex-wrap items-baseline gap-x-3 gap-y-1'>
          <h3 className='theme-eco-heading'>{t(`${page}.funds.title`, {})}</h3>
          <span className='theme-eco-heading'>{funds === null ? '…' : formatEuro(funds, language)}</span>
          <AddFundsLink />
        </div>
        <p className='theme-eco-text'>{t(`${page}.funds.note`, {})}</p>
      </div>
    </section>
  );
}

/** The account's paid servers, each with its month and the owner's buttons. */
function PaidServers({ servers }: { servers: PaidServer[] }) {
  const { t, language } = useTranslations();

  if (servers.length === 0) {
    return null;
  }

  return (
    <section className='flex flex-col gap-3'>
      <h2 className='theme-eco-heading'>{t(`${page}.servers.title`, {})}</h2>
      <div className='theme-eco-card'>
        <div className='theme-rows'>
          {servers.map((server) => {
            const price = t(`${page}.perMonth`, { price: formatEuro(server.price, language) });

            return (
              <div key={server.uuid} className='theme-row'>
                <div className='theme-row-text'>
                  <Link to={`/server/${server.uuidShort}`} className='theme-row-title theme-eco-link'>
                    {server.name}
                  </Link>
                  <p className='theme-row-body'>{server.plan ? `${server.plan} · ${price}` : price}</p>
                  <PaidStatus subscription={server} />
                </div>
                <div className='theme-row-actions'>
                  <PaidBillingActions serverUuid={server.uuid} serverName={server.name} subscription={server} />
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

export default function DashboardDedicated() {
  const { t, language } = useTranslations();
  const navigate = useNavigate();
  const name = useGlobalStore((state) => state.settings.games?.dedicatedName) || 'Paid';
  const accountsUrl = useGlobalStore((state) => state.settings.noprofit?.accountsUrl);

  const resource = useResource({
    queryKey: queryKeys.user.servers.dedicated(),
    queryFn: getDedicatedPlans,
  });

  // the plan being ordered here, paid with Funds
  const [ordering, setOrdering] = useState<{ plan: DedicatedPlan; price: number } | null>(null);
  const selfService = resource.data?.selfService ?? false;

  const price = (cents: number) => formatEuro(cents, language);

  const size = (plan: DedicatedPlan) =>
    t(`${page}.size`, {
      threads: String(plan.threads),
      memory: String(plan.memory),
      disk: String(plan.disk),
    });

  const order = (plan: DedicatedPlan) => {
    if (selfService && plan.price !== null) {
      setOrdering({ plan, price: plan.price });
      return;
    }

    const body = [
      t(`${page}.orderBody`, { name, plan: plan.name }),
      size(plan),
      plan.price !== null ? t(`${page}.perMonth`, { price: price(plan.price) }) : null,
      t(`${page}.orderLocations`, { locations: plan.locations.map((location) => location.name).join(', ') }),
    ]
      .filter(Boolean)
      .join('\n');

    const params = new URLSearchParams({ subject: t(`${page}.orderSubject`, { name, plan: plan.name }), body });
    navigate(`/account/support?${params.toString()}`);
  };

  return (
    <AccountContentContainer
      title={name}
      subtitle={t(selfService ? `${page}.subtitle` : `${page}.description`, { name })}
    >
      {ordering && (
        <DedicatedOrderModal plan={ordering.plan} price={ordering.price} onClose={() => setOrdering(null)} />
      )}
      <ResourceView resource={resource}>
        {({ plans, servers }) => (
          <div className='flex flex-col gap-6'>
            <section className='flex flex-col gap-2'>
              <h2 className='theme-eco-heading'>{t(`${page}.benefits.title`, {})}</h2>
              <ul className='theme-eco-text list-disc ps-5'>
                {BENEFITS.map((benefit) => (
                  <li key={benefit}>{t(`${page}.benefits.${benefit}`, {})}</li>
                ))}
              </ul>
            </section>

            <FundsCard selfService={selfService} />

            <PaidServers servers={servers} />

            <section className='flex flex-col gap-3'>
              <h2 className='theme-eco-heading'>{t(`${page}.plans.title`, {})}</h2>
              {plans.length === 0 ? (
                <Text c='dimmed'>{t(`${page}.empty`, {})}</Text>
              ) : (
                <div className='theme-choice-grid theme-choice-grid-3'>
                  {plans.map((plan) => {
                    const available = plan.locations.some((location) => location.available);

                    return (
                      <section key={plan.uuid} className='theme-eco-card justify-between'>
                        <div className='flex flex-col gap-2'>
                          <h3 className='theme-eco-heading'>{plan.name}</h3>
                          {plan.description && <p className='theme-eco-text'>{plan.description}</p>}
                          {plan.price !== null && (
                            <p className='text-2xl font-semibold'>
                              {t(`${page}.perMonth`, { price: price(plan.price) })}
                            </p>
                          )}
                          <ul className='theme-eco-text flex flex-col gap-1'>
                            <li>{t(`${page}.spec.threads`, { threads: String(plan.threads) })}</li>
                            <li>{t(`${page}.spec.memory`, { memory: bytesToString(mbToBytes(plan.memory)) })}</li>
                            <li>{t(`${page}.spec.disk`, { disk: bytesToString(mbToBytes(plan.disk)) })}</li>
                            {plan.locations.length > 0 && (
                              <li>
                                {plan.locations
                                  .map((location) =>
                                    location.available
                                      ? location.name
                                      : t(`${page}.soldOutAt`, { location: location.name }),
                                  )
                                  .join(' · ')}
                              </li>
                            )}
                          </ul>
                        </div>
                        <Tooltip label={t(`${page}.soldOut`, {})} disabled={available}>
                          <span>
                            <Button className='w-full' disabled={!available} onClick={() => order(plan)}>
                              {selfService && plan.price !== null ? t(`${page}.orderPaid`, {}) : t(`${page}.order`, {})}
                            </Button>
                          </span>
                        </Tooltip>
                      </section>
                    );
                  })}
                </div>
              )}
            </section>

            {selfService && accountsUrl && (
              <a
                href={`${accountsUrl.replace(/\/+$/, '')}/funds`}
                target='_blank'
                rel='noopener noreferrer'
                className='theme-eco-link self-start text-sm'
              >
                {t(`${page}.history`, {})}
              </a>
            )}
          </div>
        )}
      </ResourceView>
    </AccountContentContainer>
  );
}
