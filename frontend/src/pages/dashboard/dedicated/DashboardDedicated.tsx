// plugged.host: the paid plans whose CPU threads and memory are the server's alone, on a page of their
// own apart from the free ones, ordered from the team with a support ticket and set up by an admin
// (docs/features/dedicated.md)
import { useNavigate } from 'react-router';
import { z } from 'zod';
import getDedicatedPlans from '@/api/server/getDedicatedPlans.ts';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Choice from '@/elements/input/Choice.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverCreateDedicatedPlanSchema } from '@/lib/schemas/server/create.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type DedicatedPlan = z.infer<typeof serverCreateDedicatedPlanSchema>;

const page = 'pages.account.dedicated';

export default function DashboardDedicated() {
  const { t, language } = useTranslations();
  const navigate = useNavigate();
  const name = useGlobalStore((state) => state.settings.games?.dedicatedName) || 'Dedicated';

  const resource = useResource({
    queryKey: queryKeys.user.servers.dedicated(),
    queryFn: getDedicatedPlans,
  });

  const price = (cents: number) =>
    new Intl.NumberFormat(intlLocale(language), { style: 'currency', currency: 'EUR' }).format(cents / 100);

  const size = (plan: DedicatedPlan) =>
    t(`${page}.size`, {
      threads: String(plan.threads),
      memory: String(plan.memory),
      disk: String(plan.disk),
    });

  const order = (plan: DedicatedPlan) => {
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
    <AccountContentContainer title={name} subtitle={t(`${page}.description`, { name })}>
      <ResourceView resource={resource}>
        {(plans) =>
          plans.length === 0 ? (
            <Text c='dimmed'>{t(`${page}.empty`, {})}</Text>
          ) : (
            <div className='theme-choice-grid theme-choice-grid-3'>
              {plans.map((plan) => {
                const available = plan.locations.some((location) => location.available);

                return (
                  <Choice
                    key={plan.uuid}
                    locked
                    selected={false}
                    disabled={!available}
                    onSelect={() => available && order(plan)}
                  >
                    <span className='theme-choice-title'>{plan.name}</span>
                    {plan.price !== null && (
                      <span className='theme-choice-text'>{t(`${page}.perMonth`, { price: price(plan.price) })}</span>
                    )}
                    {plan.description && <span className='theme-choice-text'>{plan.description}</span>}
                    <span className='theme-choice-meta'>{size(plan)}</span>
                    {plan.locations.length > 0 && (
                      <span className='theme-choice-meta'>
                        {plan.locations
                          .map((location) =>
                            location.available ? location.name : t(`${page}.soldOutAt`, { location: location.name }),
                          )
                          .join(' · ')}
                      </span>
                    )}
                    <span className='theme-choice-text'>
                      {available ? t(`${page}.order`, {}) : t(`${page}.soldOut`, {})}
                    </span>
                  </Choice>
                );
              })}
            </div>
          )
        }
      </ResourceView>
    </AccountContentContainer>
  );
}
