// plugged.host: a paid server's month as the Paid page and Settings › Billing show it: the status line, the renewal toggle and "Pay next month now" (docs/features/dedicated.md)
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import payServerBillingAhead from '@/api/server/billing/payServerBillingAhead.ts';
import updateServerBilling from '@/api/server/billing/updateServerBilling.ts';
import { formatEuro } from '@/elements/AddFundsLink.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { ServerSubscription } from '@/lib/schemas/server/billing.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const base = 'pages.server.settings.billing';

/** Days a paid month runs (MONTH_DAYS in shared/src/dedicated.rs). */
const MONTH_DAYS = 30;

const day = (date: Date) => formatDateTime(date, false, false);

/** "Paid until …", "Ends on …" or, in red, when it is deleted unless paid. */
export function PaidStatus({ subscription }: { subscription: ServerSubscription }) {
  const { t } = useTranslations();

  if (subscription.unpaidSince && subscription.deleteAt) {
    return (
      <Text size='sm' c='red'>
        {t(`${base}.unpaid`, { since: day(subscription.unpaidSince), date: day(subscription.deleteAt) })}
      </Text>
    );
  }

  return (
    <Text size='sm' c='dimmed'>
      {subscription.renews
        ? t(`${base}.paidUntil`, { date: day(subscription.paidUntil) })
        : t(`${base}.ends`, { date: day(subscription.paidUntil) })}
    </Text>
  );
}

/**
 * The owner's buttons: renewal on or off (off asks first, with the end date; on after the month
 * ended pays it now) and paying the next month now (asks first, with the price).
 */
export function PaidBillingActions({
  serverUuid,
  serverName,
  subscription,
  onChanged,
}: {
  serverUuid: string;
  serverName: string;
  subscription: ServerSubscription;
  /** After a change; the server's billing and the Paid page's list are refreshed already. */
  onChanged?: () => void;
}) {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [confirming, setConfirming] = useState<'renewal' | 'pay' | null>(null);
  const [loading, setLoading] = useState(false);

  const price = formatEuro(subscription.price, language);
  const paidUntil = day(subscription.paidUntil);
  // paid ahead the month follows on; unpaid it starts now
  const nextUntil = day(new Date(Math.max(subscription.paidUntil.getTime(), Date.now()) + MONTH_DAYS * 86_400_000));

  const changed = (fresh: { subscription: ServerSubscription | null }) => {
    queryClient.setQueryData(queryKeys.server(serverUuid).billing(), fresh);
    queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
    queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.dedicated() });
    queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.all() });
    onChanged?.();
  };

  const renewalOff = async () => {
    changed(await updateServerBilling(serverUuid, false));
    setConfirming(null);
    addToast(t(`${base}.toast.off`, { date: paidUntil }), 'success');
  };

  const renewalOn = () => {
    setLoading(true);
    updateServerBilling(serverUuid, true)
      .then((fresh) => {
        changed(fresh);
        addToast(t(`${base}.toast.on`, {}), 'success');
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
      })
      .finally(() => setLoading(false));
  };

  const payAhead = async () => {
    const fresh = await payServerBillingAhead(serverUuid);
    changed(fresh);
    setConfirming(null);
    addToast(
      t(`${base}.toast.paid`, {
        price,
        date: fresh.subscription ? day(fresh.subscription.paidUntil) : nextUntil,
      }),
      'success',
    );
  };

  return (
    <>
      <ConfirmationModal
        opened={confirming === 'renewal'}
        onClose={() => setConfirming(null)}
        title={t(`${base}.confirm.title`, {})}
        confirm={t(`${base}.confirm.button`, {})}
        onConfirmed={renewalOff}
      >
        {t(`${base}.confirm.body`, { name: serverName, date: paidUntil })}
      </ConfirmationModal>
      <ConfirmationModal
        opened={confirming === 'pay'}
        onClose={() => setConfirming(null)}
        title={t(`${base}.confirmPay.title`, {})}
        confirm={t(`${base}.confirmPay.button`, { price })}
        confirmColor='blue'
        onConfirmed={payAhead}
      >
        {t(`${base}.confirmPay.body`, { price, name: serverName, date: nextUntil })}
      </ConfirmationModal>

      <div className='flex flex-wrap gap-2'>
        {subscription.renews ? (
          <Button variant='default' size='sm' onClick={() => setConfirming('renewal')}>
            {t(`${base}.renewalOn`, {})}
          </Button>
        ) : (
          <Button variant='default' size='sm' loading={loading} onClick={renewalOn}>
            {t(`${base}.renewalOff`, {})}
          </Button>
        )}
        <Button size='sm' onClick={() => setConfirming('pay')}>
          {t(`${base}.payAhead`, {})}
        </Button>
      </div>
    </>
  );
}
