// plugged.host: the panel's own monthly price, paid with tokens (docs/features/economy.md).
import { faCoins, faLock } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { httpErrorToHuman } from '@/api/axios.ts';
import buyPanelAccess from '@/api/me/buyPanelAccess.ts';
import getPanelAccess from '@/api/me/getPanelAccess.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

const page = 'pages.account.panelAccess';

export default function DashboardPanelAccess() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [buying, setBuying] = useState(false);
  // plugged.host: both memberships include the panel (docs/features/promos.md#offers)
  const offers = useGlobalStore((state) => !!state.settings.ads?.offers);
  const economy = useGlobalStore((state) => state.settings.economy);
  const membershipLine = offers && !!economy?.membershipEnabled;

  const access = useResource({
    queryKey: queryKeys.user.panelAccess(),
    queryFn: getPanelAccess,
  });

  const doBuy = async () => {
    setBuying(true);
    try {
      const next = await buyPanelAccess();
      queryClient.setQueryData(queryKeys.user.panelAccess(), next);
      queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
      queryClient.invalidateQueries({ queryKey: queryKeys.user.tokens() });
      queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.all() });
      addToast(t(`${page}.toast.bought`, { days: next.days }), 'success');

      const redirect = sessionStorage.getItem('panel-access-redirect');
      sessionStorage.removeItem('panel-access-redirect');
      navigate(redirect && !redirect.startsWith('/account/panel-access') ? redirect : '/');
    } catch (err) {
      addToast(httpErrorToHuman(err), 'error');
    } finally {
      setBuying(false);
    }
  };

  return (
    <AccountContentContainer title={t(`${page}.title`, {})}>
      <ResourceView resource={access}>
        {(panelAccess) => (
          <section className='theme-eco-card'>
            <div className='flex items-center gap-3'>
              <FontAwesomeIcon icon={faLock} size='lg' />
              <div className='flex flex-col gap-1'>
                <h3 className='theme-eco-heading'>
                  {t(`${page}.heading`, { tokens: panelAccess.tokens, days: panelAccess.days })}
                </h3>
                <p className='theme-eco-muted'>
                  {panelAccess.exempt
                    ? t(`${page}.exempt`, {})
                    : panelAccess.until
                      ? t(`${page}.active`, { date: formatDateTime(panelAccess.until) })
                      : t(`${page}.inactive`, {})}
                </p>
              </div>
            </div>

            {!panelAccess.enabled ? (
              <Notification color='blue'>{t(`${page}.disabled`, {})}</Notification>
            ) : panelAccess.exempt ? (
              <Notification color='green'>{t(`${page}.exemptNotice`, {})}</Notification>
            ) : (
              <>
                <div className='flex flex-wrap items-center gap-2'>
                  <Button onClick={doBuy} loading={buying} leftSection={<FontAwesomeIcon icon={faCoins} />}>
                    {t(`${page}.buy`, { tokens: panelAccess.tokens })}
                  </Button>
                  <Button onClick={() => navigate('/account/tokens')} variant='light'>
                    {t(`${page}.earn`, {})}
                  </Button>
                </div>
                {membershipLine && economy && (
                  <p className='theme-eco-small'>
                    {t(`${page}.offers.membership`, { plus: economy.plusName, plusplus: economy.plusplusName })}{' '}
                    <Link to='/account/membership' className='theme-eco-link'>
                      {t(`${page}.offers.membershipLink`, { name: economy.plusName })}
                    </Link>
                  </p>
                )}
              </>
            )}
          </section>
        )}
      </ResourceView>
    </AccountContentContainer>
  );
}
