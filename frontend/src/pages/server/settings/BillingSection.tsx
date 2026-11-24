// plugged.host: Settings' "Billing" for a server paid monthly with noprofit.space Funds: its price, until when it is paid, its renewal and paying the next month now, which only the owner changes (docs/features/dedicated.md)
import { Link } from 'react-router';
import { formatEuro } from '@/elements/AddFundsLink.tsx';
import Text from '@/elements/typography/Text.tsx';
import { PaidBillingActions, PaidStatus } from '@/pages/dashboard/dedicated/PaidBilling.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import SettingsSection from './SettingsSection.tsx';
import { useServerBilling } from './useSettingsSections.ts';

const base = 'pages.server.settings.billing';

export default function BillingSection() {
  const { t, language } = useTranslations();
  const server = useServerStore((state) => state.server);
  const billing = useServerBilling();

  const subscription = billing.data?.subscription;
  if (!server.isDedicated || !subscription) {
    return null;
  }

  return (
    <SettingsSection id='billing' title={t(`${base}.title`, {})} description={t(`${base}.note`, {})}>
      <div className='theme-eco-card'>
        <div className='flex flex-col gap-3'>
          <div className='flex flex-col gap-1'>
            <h3 className='theme-row-title'>
              {t(`${base}.price`, { price: formatEuro(subscription.price, language) })}
            </h3>
            <PaidStatus subscription={subscription} />
          </div>
          {server.isOwner ? (
            <PaidBillingActions serverUuid={server.uuid} serverName={server.name} subscription={subscription} />
          ) : (
            <Text size='sm' c='dimmed'>
              {t(`${base}.ownerOnly`, {})}
            </Text>
          )}
          <Link to='/account/dedicated' className='theme-eco-link self-start text-sm'>
            {t(`${base}.allServers`, {})}
          </Link>
        </div>
      </div>
    </SettingsSection>
  );
}
