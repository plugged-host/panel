// plugged.host: the Transfer page while a move runs, like the Pelican panel's progress card: the
// route, the state with its percent and bar, the steps, the facts, and Cancel (docs/features/transfers.md)
import { faCheck } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { z } from 'zod';
import { getHttpStatus, httpErrorToHuman } from '@/api/axios.ts';
import cancelServerTransfer from '@/api/server/transfer/cancelServerTransfer.ts';
import Button from '@/elements/buttons/Button.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { shortDuration, TRANSFER_STEPS, transferStateText } from '@/lib/domain/transfer.ts';
import { bytesToString } from '@/lib/format/size.ts';
import { serverTransferOptionsSchema, serverTransferRequestSchema } from '@/lib/schemas/server/transfer.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { TransferFlag } from './TransferParts.tsx';

type Options = z.infer<typeof serverTransferOptionsSchema>;
type Request = z.infer<typeof serverTransferRequestSchema>;

const page = 'pages.server.transfer';

/** States in which Wings can still drop the copy (not once the new node is checking it). */
const CANCELLABLE = ['queued', 'preparing', 'retrying', 'stopping', 'copying'];

export default function TransferProgress({
  options,
  request,
  refresh,
}: {
  options: Options;
  request: Request;
  refresh: () => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const canAdminTransfer = useAdminCan('servers.transfer');

  const [confirming, setConfirming] = useState(false);

  const progress = options.progress;
  const state = request.status === 'cancelling' ? 'cancelling' : (progress?.state ?? 'preparing');
  const percent = Math.min(100, Math.max(0, progress?.percent ?? 0));
  const current = TRANSFER_STEPS.findIndex((step) => step === progress?.step);
  // a move an admin started (Lighten Load, Admin › Servers) is only cancelled by an admin
  const mayCancel = CANCELLABLE.includes(state) && (request.origin === 'user' || canAdminTransfer);

  const doCancel = async () => {
    await cancelServerTransfer(server.uuid)
      .then(() => addToast(t(`${page}.toast.cancelled`, {}), 'success'))
      .catch((error) =>
        addToast(
          getHttpStatus(error) === 409 ? t(`${page}.toast.cancelFailed`, {}) : httpErrorToHuman(error),
          'warning',
        ),
      )
      .finally(() => {
        setConfirming(false);
        refresh();
      });
  };

  return (
    <section className='theme-eco-card'>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t(`${page}.cancelModal.title`, {})}
        confirm={t(`${page}.button.cancel`, {})}
        confirmColor='gray'
        onConfirmed={doCancel}
      >
        {t(`${page}.cancelModal.content`, {})}
      </ConfirmationModal>

      <div className='theme-transfer-route'>
        <div className='theme-transfer-place'>
          <TransferFlag place={request.fromLocation} />
          <div>
            <small className='theme-eco-small block'>{t(`${page}.from`, {})}</small>
            <strong>{request.fromLocation?.name ?? t(`${page}.unknownLocation`, {})}</strong>
          </div>
        </div>
        <span className='theme-transfer-arrow' aria-hidden>
          →
        </span>
        <div className='theme-transfer-place theme-transfer-place-to'>
          <div>
            <small className='theme-eco-small block'>{t(`${page}.to`, {})}</small>
            <strong>{request.toLocation?.name ?? t(`${page}.unknownLocation`, {})}</strong>
          </div>
          <TransferFlag place={request.toLocation} />
        </div>
      </div>

      <div>
        <div className='theme-transfer-status'>
          <div className='theme-transfer-headline'>
            <Spinner size={16} />
            <span>{transferStateText(t, state)}</span>
          </div>
          <div className='theme-transfer-pct font-mono'>{percent}%</div>
        </div>
        <div className='theme-transfer-bar'>
          <span style={{ width: `${percent}%` }} />
        </div>
      </div>

      <div className='theme-transfer-steps'>
        {TRANSFER_STEPS.map((step, index) => {
          const status = index < current ? 'done' : index === current ? 'current' : 'todo';

          return (
            <div key={step} className='theme-transfer-step' data-status={status}>
              <span>{status === 'done' ? <FontAwesomeIcon icon={faCheck} /> : index + 1}</span>
              <small>{t(`${page}.steps.${step}`, {})}</small>
            </div>
          );
        })}
      </div>

      {progress && (
        <div className='theme-transfer-facts'>
          {progress.bytesTotal > 0 && (
            <div className='theme-transfer-fact'>
              <small className='theme-eco-small block'>{t(`${page}.data`, {})}</small>
              <strong className='font-mono'>
                {bytesToString(progress.bytesDone)} / {bytesToString(progress.bytesTotal)}
              </strong>
            </div>
          )}
          {progress.rate !== null && (
            <div className='theme-transfer-fact'>
              <small className='theme-eco-small block'>{t(`${page}.speed`, {})}</small>
              <strong className='font-mono'>{bytesToString(progress.rate)}/s</strong>
            </div>
          )}
          {progress.etaSeconds !== null && (
            <div className='theme-transfer-fact'>
              <small className='theme-eco-small block'>{t(`${page}.eta`, {})}</small>
              <strong>~{shortDuration(progress.etaSeconds)}</strong>
            </div>
          )}
          {request.attempts > 1 && (
            <div className='theme-transfer-fact'>
              <small className='theme-eco-small block'>{t(`${page}.attempt`, {})}</small>
              <strong>
                {t(`${page}.attemptValue`, {
                  attempt: request.attempts,
                  max: Math.max(options.maxAttempts, request.attempts),
                })}
              </strong>
            </div>
          )}
        </div>
      )}

      <div className='theme-transfer-foot'>
        <p className='theme-eco-muted'>{t(`${page}.safe`, {})}</p>
        {mayCancel && (
          <Button color='gray' size='sm' onClick={() => setConfirming(true)}>
            {t(`${page}.button.cancel`, {})}
          </Button>
        )}
      </div>
    </section>
  );
}
