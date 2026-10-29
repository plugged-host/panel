// plugged.host: a start waiting for room on a busy node, laid out like the Pelican panel's queue
// page: the place in line, why, the ways out and the ads (docs/features/start-queue.md)
import { AxiosError } from 'axios';
import { useEffect, useState } from 'react';
import { Navigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getStartQueue from '@/api/server/queue/getStartQueue.ts';
import leaveStartQueue from '@/api/server/queue/leaveStartQueue.ts';
import takeQueueMove from '@/api/server/queue/takeQueueMove.ts';
import { AdStack } from '@/elements/ads/AdSlot.tsx';
import Button from '@/elements/buttons/Button.tsx';
import UnstyledButton from '@/elements/buttons/UnstyledButton.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverStartQueueSchema } from '@/lib/schemas/server/queue.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { formatWaited, useLeaveWaitingPage, useWaitClock } from '@/plugins/server/useWaitingPage.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Queue = z.infer<typeof serverStartQueueSchema>;

const page = 'pages.server.queue';

/** One node reading: its label, the figure (red over the line) and a thin bar. */
function Meter({ label, value, line }: { label: string; value: number | null; line: number }) {
  const shown = value === null ? null : Math.min(100, Math.max(0, value));
  const over = shown !== null && shown > line;

  return (
    <div className='theme-wait-meter'>
      <div className='theme-wait-meter-head'>
        <span className='theme-eco-small'>{label}</span>
        <span className='theme-wait-meter-value' data-over={over || undefined}>
          {shown === null ? '—' : `${shown.toFixed(1)}%`}
        </span>
      </div>
      <div className='theme-wait-bar'>
        <span data-over={over || undefined} style={{ width: `${Math.round(shown ?? 0)}%` }} />
      </div>
    </div>
  );
}

function Waiting({ queue, refetch }: { queue: Queue; refetch: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const leavePage = useLeaveWaitingPage();
  const server = useServerStore((state) => state.server);
  const appName = useGlobalStore((state) => state.settings.app.name);
  const economy = useGlobalStore((state) => state.settings.economy);
  const offers = useGlobalStore((state) => !!state.settings.ads?.offers); // plugged.host

  const [confirmingMove, setConfirmingMove] = useState(false);
  const [leaving, setLeaving] = useState(false);

  // reaching the longest wait asks the panel at once: the release lands when the page says it does
  const clock = useWaitClock(queue.waited, queue.maxWait, refetch);
  const consolePath = `/server/${server.uuidShort}`;
  const plusName = economy?.plusName ?? '';
  const plusplusName = economy?.plusplusName ?? '';

  // plugged.host: the second membership goes first; one line for whoever does not have it
  // (docs/features/promos.md#offers)
  const premiumLine =
    offers && economy?.membershipEnabled && queue.priority < 2 ? (
      <p className='theme-eco-small'>
        {t(`${page}.offers.premium`, { name: plusplusName })}{' '}
        <a
          className='theme-eco-link'
          href='/account/membership'
          onClick={(event) => {
            event.preventDefault();
            leavePage('/account/membership');
          }}
        >
          {t(`${page}.offers.premiumLink`, { name: plusplusName })}
        </a>
      </p>
    ) : null;

  // giving the place up: back to the console without starting anything
  const doLeave = () => {
    setLeaving(true);

    leaveStartQueue(server.uuid)
      .then(() => leavePage(consolePath))
      .catch((err) => {
        // already out of the line (let go, or left): there is nothing to give up
        if (err instanceof AxiosError && err.response?.status === 409) {
          leavePage(consolePath);
          return;
        }
        addToast(httpErrorToHuman(err), 'error');
        setLeaving(false);
      });
  };

  const doMove = async () => {
    try {
      await takeQueueMove(server.uuid);
    } catch (err) {
      setConfirmingMove(false);
      refetch(); // the node named on the offer may have changed
      throw err;
    }

    addToast(t(`${page}.toast.moveStarted`, {}), 'success');
    leavePage(`${consolePath}/transfer`);
  };

  return (
    <div className='theme-wait-page'>
      {/* where the start is in the line, how long it waited, and the longest it waits */}
      <section className='theme-eco-card'>
        <div className='theme-wait-head'>
          <div className='flex min-w-0 flex-col gap-1'>
            <div className='theme-wait-title'>
              <Spinner size={20} />
              <h3 className='theme-eco-heading truncate'>{t(`${page}.heading`, {})}</h3>
            </div>
            <p className='theme-row-body'>{t(`${page}.body.${queue.action}`, {})}</p>
          </div>

          <div className='theme-wait-figure'>
            <span className='theme-caption'>{t(`${page}.position.label`, {})}</span>
            <span className='theme-figure theme-figure-accent'>
              {queue.position > 1
                ? t(`${page}.position.value`, { position: queue.position })
                : t(`${page}.position.next`, {})}
            </span>
            {queue.priority > 0 && <span className='theme-wait-priority'>{t(`${page}.position.priority`, {})}</span>}
          </div>
        </div>

        <div className='theme-wait-track'>
          <div className='theme-wait-fill' style={{ width: `${clock.percent}%` }} />
        </div>

        <div className='theme-wait-times theme-eco-small'>
          <span>{t(`${page}.waited`, { time: formatWaited(clock.elapsed) })}</span>
          <span>
            {clock.remaining > 0 ? t(`${page}.capNote`, { seconds: clock.remaining }) : t(`${page}.capReached`, {})}
          </span>
        </div>
      </section>

      {/* why: the node's own reading, the one the release goes by */}
      {(queue.memoryPercent !== null || queue.cpuPercent !== null) && (
        <section className='theme-eco-card'>
          <p className='theme-row-body'>
            {queue.reason
              ? t(`${page}.reason.${queue.reason}`, { limit: Math.round(queue.line) })
              : t(`${page}.reason.cleared`, {})}
          </p>

          <div className='theme-wait-meters'>
            <Meter label={t(`${page}.usage.memory`, {})} value={queue.memoryPercent} line={queue.line} />
            <Meter label={t(`${page}.usage.cpu`, {})} value={queue.cpuPercent} line={queue.line} />
          </div>
        </section>
      )}

      <AdStack part='above' />

      {queue.priority > 0 ? (
        <Notification
          className='theme-callout'
          color='green'
          title={
            queue.priority >= 2
              ? t(`${page}.priority.plusplus.heading`, { app: appName, name: plusplusName })
              : t(`${page}.priority.plus.heading`, { app: appName, name: plusName })
          }
        >
          {queue.priority >= 2
            ? t(`${page}.priority.plusplus.body`, { name: plusplusName, plus: plusName })
            : t(`${page}.priority.plus.body`, { name: plusName })}
          {premiumLine}
        </Notification>
      ) : queue.membershipOffer ? (
        <section className='theme-eco-card'>
          <div className='theme-row'>
            <div className='flex min-w-0 flex-col gap-1'>
              <h3 className='theme-eco-heading'>{t(`${page}.plusOffer.heading`, { app: appName, name: plusName })}</h3>
              <p className='theme-row-body'>{t(`${page}.plusOffer.body`, { name: plusName, cost: queue.plusCost })}</p>
              {premiumLine}
            </div>
            <div className='theme-row-action'>
              <Button className='w-full' onClick={() => leavePage('/account/membership')}>
                {t(`${page}.plusOffer.button`, { name: plusName })}
              </Button>
            </div>
          </div>
        </section>
      ) : null}

      {queue.moveNode ? (
        <section className='theme-eco-card'>
          <ConfirmationModal
            opened={confirmingMove}
            onClose={() => setConfirmingMove(false)}
            title={t(`${page}.move.modal.title`, { node: queue.moveNode })}
            confirm={t(`${page}.move.button`, {})}
            confirmColor='blue'
            onConfirmed={doMove}
          >
            {t(`${page}.move.modal.content`, {})}
          </ConfirmationModal>

          <div className='theme-row'>
            <div className='flex min-w-0 flex-col gap-1'>
              <h3 className='theme-eco-heading'>{t(`${page}.move.heading`, {})}</h3>
              <p className='theme-row-body'>{t(`${page}.move.body`, { node: queue.moveNode })}</p>
              <p className='theme-eco-small'>{t(`${page}.move.note`, {})}</p>
            </div>
            <div className='theme-row-action'>
              <Button color='gray' className='w-full' onClick={() => setConfirmingMove(true)}>
                {t(`${page}.move.button`, {})}
              </Button>
            </div>
          </div>
        </section>
      ) : queue.moveTooBig ? (
        <p className='theme-eco-small'>{t(`${page}.move.tooBig`, {})}</p>
      ) : null}

      <AdStack part='below' />

      {/* leaving properly: quiet, at the bottom, and a link rather than a button (Filament's gray
          link: ".theme-link-action" of the table header and record actions block) */}
      <div className='flex flex-col items-center gap-1 pb-2 text-center'>
        <UnstyledButton className='theme-link-action' disabled={leaving} onClick={doLeave}>
          {t(`${page}.leave`, {})}
        </UnstyledButton>
        {queue.passMinutes > 0 && (
          <span className='theme-row-faint'>{t(`${page}.passNote`, { minutes: queue.passMinutes })}</span>
        )}
      </div>
    </div>
  );
}

export default function ServerQueue() {
  const { t } = useTranslations();
  const leavePage = useLeaveWaitingPage();
  const server = useServerStore((state) => state.server);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).queue(),
    queryFn: () => getStartQueue(server.uuid),
  });
  const status = resource.data?.status;

  // the release happens on the panel: follow it, then go to the console the server starts in
  useEffect(() => {
    if (status !== 'waiting') return;

    const interval = setInterval(() => resource.refetch(), 5_000);
    return () => clearInterval(interval);
  }, [status]);

  useEffect(() => {
    if (status === 'released') leavePage(`/server/${server.uuidShort}`);
  }, [status]);

  return (
    <ServerContentContainer title={t(`${page}.title`, {})} subtitle={t(`${page}.subtitle`, { server: server.name })}>
      <ResourceView resource={resource}>
        {(queue) =>
          queue.status === 'waiting' ? (
            <Waiting queue={queue} refetch={resource.refetch} />
          ) : queue.status === 'released' ? null : (
            // nothing to wait for: it left the line, or never joined it
            <Navigate to={`/server/${server.uuidShort}`} replace />
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
