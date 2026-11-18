// plugged.host: Settings' "Billing" for a server paid monthly with noprofit.space Funds: its price, until when it is paid, and its renewal, which only the owner changes (docs/features/dedicated.md)
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import updateServerBilling from '@/api/server/billing/updateServerBilling.ts';
import { formatEuro } from '@/elements/AddFundsLink.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import SettingsSection from './SettingsSection.tsx';
import { useServerBilling } from './useSettingsSections.ts';

const base = 'pages.server.settings.billing';

export default function BillingSection() {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const billing = useServerBilling();
  const [confirming, setConfirming] = useState(false);
  const [loading, setLoading] = useState(false);

  const subscription = billing.data?.subscription;
  if (!server.isDedicated || !subscription) {
    return null;
  }

  const paidUntil = formatDateTime(subscription.paidUntil, false, false);

  const setRenews = async (renews: boolean) => {
    const fresh = await updateServerBilling(server.uuid, renews);
    queryClient.setQueryData(queryKeys.server(server.uuid).billing(), fresh);
    queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
    addToast(renews ? t(`${base}.toast.kept`, {}) : t(`${base}.toast.cancelled`, { date: paidUntil }), 'success');
  };

  const cancel = async () => {
    await setRenews(false);
    setConfirming(false);
  };

  const keep = () => {
    setLoading(true);
    setRenews(true)
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <SettingsSection id='billing' title={t(`${base}.title`, {})} description={t(`${base}.note`, {})}>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t(`${base}.confirm.title`, {})}
        confirm={t(`${base}.cancel`, {})}
        onConfirmed={cancel}
      >
        {t(`${base}.confirm.body`, { date: paidUntil })}
      </ConfirmationModal>

      <div className='theme-eco-card'>
        <div className='theme-rows'>
          <div className='theme-row'>
            <div className='theme-row-text'>
              <h3 className='theme-row-title'>
                {t(`${base}.price`, { price: formatEuro(subscription.price, language), date: paidUntil })}
              </h3>
              <p className='theme-row-body'>
                {subscription.renews ? t(`${base}.renews`, {}) : t(`${base}.ends`, { date: paidUntil })}
              </p>
              {!server.isOwner && <p className='theme-row-body'>{t(`${base}.ownerOnly`, {})}</p>}
            </div>
            {server.isOwner && (
              <div className='theme-row-actions'>
                {subscription.renews ? (
                  <Button variant='default' size='sm' onClick={() => setConfirming(true)}>
                    {t(`${base}.cancel`, {})}
                  </Button>
                ) : (
                  <Button size='sm' loading={loading} onClick={keep}>
                    {t(`${base}.keep`, {})}
                  </Button>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </SettingsSection>
  );
}
