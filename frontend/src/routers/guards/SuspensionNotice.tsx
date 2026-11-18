// plugged.host: what the owner and subusers of a suspended server read (docs/features/suspension.md).
import { Button as MantineButton } from '@mantine/core';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getBalance from '@/api/me/economy/getBalance.ts';
import getServerBilling from '@/api/server/billing/getServerBilling.ts';
import updateServerBilling from '@/api/server/billing/updateServerBilling.ts';
import getServer from '@/api/server/getServer.ts';
import AddFundsLink, { formatEuro } from '@/elements/AddFundsLink.tsx';
import Button from '@/elements/buttons/Button.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverSchema } from '@/lib/schemas/server/server.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

/**
 * The time left until `deleteAt` in words ("in 3 days", "in 5 hours", "now"). Rounded down, so it
 * never says more time is left than there is; the exact date is shown beside it.
 */
function deletionIn(deleteAt: Date, now: number, language: string): string {
  const seconds = Math.floor((deleteAt.getTime() - now) / 1000);
  const locale = intlLocale(language);

  // less than a minute left, or due: the purge deletes it on its next run (every 5 minutes)
  if (seconds < 60) {
    return new Intl.RelativeTimeFormat(locale, { numeric: 'auto' }).format(0, 'second');
  }

  // always a number: rounded-down days are not calendar days, so "tomorrow" could be wrong
  const format = new Intl.RelativeTimeFormat(locale, { numeric: 'always' });
  if (seconds < 3_600) {
    return format.format(Math.floor(seconds / 60), 'minute');
  }
  if (seconds < 172_800) {
    return format.format(Math.floor(seconds / 3_600), 'hour');
  }

  return format.format(Math.floor(seconds / 86_400), 'day');
}

const unpaidBase = 'elements.screenBlock.serverConflict.suspension.unpaid';

/**
 * A paid server suspended because its month could not be paid: its owner pays it now from
 * noprofit.space Funds, which lifts the suspension (docs/features/dedicated.md).
 */
function PayNow({ server }: { server: z.infer<typeof serverSchema> }) {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const updateServer = useServerStore((state) => state.updateServer);
  const [loading, setLoading] = useState(false);

  const billing = useQuery({
    queryKey: queryKeys.server(server.uuid).billing(),
    queryFn: () => getServerBilling(server.uuid),
  });
  const balance = useQuery({
    queryKey: queryKeys.user.balance(),
    queryFn: getBalance,
    refetchInterval: 60_000,
  });

  const price = billing.data?.subscription?.price ?? null;
  const funds = balance.data ? (balance.data.funds ?? 0) : null;
  const short = price !== null && funds !== null && funds < price;

  const pay = () => {
    setLoading(true);
    updateServerBilling(server.uuid, true)
      .then(async (fresh) => {
        queryClient.setQueryData(queryKeys.server(server.uuid).billing(), fresh);
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
        queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.all() });
        addToast(t(`${unpaidBase}.paid`, {}), 'success');
        updateServer(await getServer(server.uuid));
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
      })
      .finally(() => setLoading(false));
  };

  return (
    <div className='flex flex-col items-start gap-2'>
      {price !== null && (
        <p className='text-(--mantine-color-text)'>
          {funds === null
            ? t(`${unpaidBase}.price`, { price: formatEuro(price, language) })
            : t(`${unpaidBase}.priceFunds`, {
                price: formatEuro(price, language),
                funds: formatEuro(funds, language),
              })}
        </p>
      )}
      {short && (
        <>
          <p>{t(`${unpaidBase}.short`, {})}</p>
          <AddFundsLink />
        </>
      )}
      <Button loading={loading} disabled={short || billing.isLoading} onClick={pay}>
        {t(`${unpaidBase}.payNow`, {})}
      </Button>
    </div>
  );
}

export default function SuspensionNotice({ server }: { server: z.infer<typeof serverSchema> }) {
  const { t, language } = useTranslations();
  const suspension = server.suspension;

  // the time left counts down while the page stays open
  const deleteAt = suspension?.deleteAt?.getTime() ?? null;
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (deleteAt === null) return;

    const interval = setInterval(() => setNow(Date.now()), 60_000);

    return () => clearInterval(interval);
  }, [deleteAt]);

  if (!suspension) {
    return t('elements.screenBlock.serverConflict.contentSuspended', {});
  }

  return (
    <div className='flex flex-col gap-2'>
      <p>
        {suspension.suspended
          ? t(`elements.screenBlock.serverConflict.suspension.by.${suspension.kind}`, {
              date: formatDateTime(suspension.suspended),
            })
          : t('elements.screenBlock.serverConflict.contentSuspended', {})}
      </p>
      {suspension.reason && (
        <p className='text-(--mantine-color-text) whitespace-pre-line break-words'>
          {t('elements.screenBlock.serverConflict.suspension.reason', { reason: suspension.reason })}
        </p>
      )}
      {/* the purge's deletion time (suspension_delete_at); none: never deleted, nothing said */}
      {suspension.deleteAt && (
        <>
          <p className='text-(--mantine-color-text)'>
            {t('elements.screenBlock.serverConflict.suspension.deletesIn', {
              time: deletionIn(suspension.deleteAt, now, language),
              date: formatDateTime(suspension.deleteAt, false, false),
            })}
          </p>
          {/* an unpaid month is no mistake to be sure of: paying it in time keeps the server */}
          {suspension.kind !== 'unpaid' && <p>{t('elements.screenBlock.serverConflict.suspension.notEarlier', {})}</p>}
        </>
      )}
      {suspension.kind === 'unpaid' && server.isOwner && <PayNow server={server} />}
      {suspension.helpUrl ? (
        <>
          <p>{t('elements.screenBlock.serverConflict.suspension.helpLink', {})}</p>
          {/* neo's support link, else the Discord invite (the Pelican panel's "Get help" action) */}
          <div>
            <MantineButton
              component='a'
              href={suspension.helpUrl}
              target='_blank'
              rel='noopener noreferrer'
              variant='default'
              fw={600}
            >
              {t('elements.screenBlock.serverConflict.suspension.getHelp', {})}
            </MantineButton>
          </div>
        </>
      ) : (
        <p>{t('elements.screenBlock.serverConflict.suspension.help', {})}</p>
      )}
    </div>
  );
}
