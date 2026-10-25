// plugged.host: the Transfer page when no move runs, like the Pelican panel's: how the last move
// ended, where the server is, a row per location, Move, and what to expect (docs/features/transfers.md)
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import requestServerTransfer from '@/api/server/transfer/requestServerTransfer.ts';
import Button from '@/elements/buttons/Button.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { formatAllocation } from '@/lib/domain/server.ts';
import {
  endedWithin,
  OUTCOME_MINUTES,
  shortDuration,
  transferBlockedText,
  transferReasonText,
  transferStateText,
} from '@/lib/domain/transfer.ts';
import { tierName } from '@/lib/economy.ts';
import { bytesToString } from '@/lib/format/size.ts';
import { serverTransferOptionsSchema } from '@/lib/schemas/server/transfer.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { TransferFlag } from './TransferParts.tsx';

type Options = z.infer<typeof serverTransferOptionsSchema>;

const page = 'pages.server.transfer';

function Fact({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className='theme-transfer-fact'>
      <small className='theme-eco-small block'>{label}</small>
      <strong className={mono ? 'font-mono' : undefined}>{value}</strong>
    </div>
  );
}

export default function TransferIdle({ options, refresh }: { options: Options; refresh: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);

  const [locationUuid, setLocationUuid] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  const recent = options.request && endedWithin(options.request, OUTCOME_MINUTES) ? options.request : null;
  const blocked = transferBlockedText(t, options.blocked, options.cooldownUntil);
  const location = options.locations.find((location) => location.uuid === locationUuid) ?? null;
  const time = shortDuration(options.estimateSeconds);
  const reason = recent?.status === 'failed' ? transferReasonText(t, recent) : null;
  const hint =
    recent?.status === 'completed'
      ? t(`${page}.stateHint.completed`, {})
      : recent?.status === 'cancelled'
        ? t(`${page}.stateHint.cancelled`, {})
        : `${t(`${page}.stateHint.failed`, {})}${reason ? ` (${reason})` : ''}`;
  // the address the move ended on, as its last step wrote it (the server's own catches up a moment later)
  const newAddress =
    options.events.find((event) => event.key === 'completed')?.params.address ??
    (server.allocation ? formatAllocation(server.allocation) : null);

  const doRequest = async () => {
    if (!location) return;

    await requestServerTransfer(server.uuid, { locationUuid: location.uuid })
      .then(() => {
        addToast(t(`${page}.toast.requested`, {}), 'success');
        setLocationUuid(null);
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => {
        setConfirming(false);
        // also refreshes the strip, whose key sits under this page's
        refresh();
      });
  };

  return (
    <>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t(`${page}.modal.title`, { location: location?.name ?? '' })}
        confirm={t(`${page}.button.move`, {})}
        confirmColor='blue'
        onConfirmed={doRequest}
      >
        {t(`${page}.modal.content`, { location: location?.name ?? '', time })}
      </ConfirmationModal>

      {recent && (
        <Notification
          className='theme-callout'
          color={recent.status === 'completed' ? 'green' : recent.status === 'cancelled' ? 'gray' : 'yellow'}
          title={transferStateText(t, recent.status)}
        >
          {hint}
          {recent.status === 'completed' && newAddress && (
            <div className='mt-2'>
              <small className='theme-eco-small block'>{t(`${page}.newAddress`, {})}</small>
              <span className='theme-transfer-addr font-mono'>{newAddress}</span>
            </div>
          )}
        </Notification>
      )}

      <section className='theme-eco-card'>
        <div className='theme-transfer-now'>
          <TransferFlag place={options.currentLocation} size='lg' />
          <div>
            <small className='theme-eco-small block'>{t(`${page}.currentLocation`, {})}</small>
            <strong>{options.currentLocation.name}</strong>
          </div>
        </div>
        <div className='theme-transfer-facts'>
          {server.allocation && (
            <Fact label={t(`${page}.address`, {})} value={formatAllocation(server.allocation)} mono />
          )}
          <Fact label={t(`${page}.data`, {})} value={options.dataBytes ? bytesToString(options.dataBytes) : '—'} mono />
          <Fact
            label={t(`${page}.backups`, {})}
            value={
              options.backups.count > 0
                ? t(`${page}.backupsValue`, {
                    count: options.backups.count,
                    size: bytesToString(options.backups.bytes),
                  })
                : t(`${page}.none`, {})
            }
          />
          <Fact label={t(`${page}.downtime`, {})} value={`~${time}`} />
        </div>
      </section>

      <section className='flex flex-col gap-3'>
        <h3 className='theme-eco-heading'>{t(`${page}.choose`, {})}</h3>

        {blocked && <p className='theme-transfer-blocked'>{blocked}</p>}

        <div className='theme-location-list'>
          {options.locations.map((option) => {
            const current = option.uuid === options.currentLocationUuid;
            const pickable = option.available && option.unlocked && !blocked;
            const selected = option.uuid === locationUuid;

            return (
              <button
                key={option.uuid}
                type='button'
                className='theme-location-row'
                data-selected={selected || undefined}
                aria-pressed={selected}
                disabled={!pickable}
                onClick={() => setLocationUuid(option.uuid)}
              >
                <TransferFlag place={option} />
                <span className='theme-location-main'>
                  {option.name}
                  {option.tier && (
                    <Badge color='gray' size='sm'>
                      {tierName(option.tier)}
                    </Badge>
                  )}
                  {current && (
                    <Badge color='gray' size='sm'>
                      {option.available ? t(`${page}.anotherNode`, {}) : t(`${page}.current`, {})}
                    </Badge>
                  )}
                  {!option.unlocked ? (
                    <Badge color='gray' size='sm'>
                      {t(`${page}.locked`, {})}
                    </Badge>
                  ) : (
                    !option.available &&
                    !current && (
                      <Badge color='gray' size='sm'>
                        {t(`${page}.full`, {})}
                      </Badge>
                    )
                  )}
                </span>
                {option.latency !== null && (
                  <span className='theme-location-ms font-mono'>{t(`${page}.ms`, { ms: option.latency })}</span>
                )}
                <span className='theme-radio' aria-hidden />
              </button>
            );
          })}
        </div>
      </section>

      <div className='theme-transfer-cta'>
        <Button size='lg' disabled={!location || !!blocked} onClick={() => setConfirming(true)}>
          {location ? t(`${page}.moveTo`, { location: location.name }) : t(`${page}.choose`, {})}
        </Button>
      </div>

      <div className='theme-transfer-help'>
        <div>
          <strong>{t(`${page}.help.safeTitle`, {})}</strong>
          <p>{t(`${page}.help.safeBody`, {})}</p>
        </div>
        <div>
          <strong>{t(`${page}.help.retryTitle`, {})}</strong>
          <p>{t(`${page}.help.retryBody`, {})}</p>
        </div>
        <div>
          <strong>{t(`${page}.help.addressTitle`, {})}</strong>
          <p>{t(`${page}.help.addressBody`, {})}</p>
        </div>
      </div>
    </>
  );
}
