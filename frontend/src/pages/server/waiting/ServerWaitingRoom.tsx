// plugged.host: a start in the waiting room, laid out like the Pelican panel's ad gate: the status card
// counting down to the console, then the ads, and no way past them (docs/features/ads.md#waiting-room)
import { AxiosError } from 'axios';
import { useEffect, useRef } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getWaitingRoom from '@/api/server/waiting/getWaitingRoom.ts';
import leaveWaitingRoom from '@/api/server/waiting/leaveWaitingRoom.ts';
import { useAdBlocked } from '@/elements/ads/AdBlockNotice.tsx';
import { AdStack } from '@/elements/ads/AdSlot.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverWaitingRoomSchema } from '@/lib/schemas/server/waitingRoom.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useHoldPage, useLeaveWaitingPage, useWaitClock } from '@/plugins/server/useWaitingPage.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Room = z.infer<typeof serverWaitingRoomSchema>;

const page = 'pages.server.waitingRoom';

function Waiting({ room, refetch, refresh }: { room: Room; refetch: () => void; refresh: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const leavePage = useLeaveWaitingPage();
  const server = useServerStore((state) => state.server);
  const blocked = useAdBlocked('server');

  const left = useRef(false);

  // counts down to the longest stay; reaching it asks the panel at once
  const clock = useWaitClock(room.elapsed, room.maxSeconds, refetch);
  const remaining = room.ready ? 0 : clock.remaining;
  const percent = room.ready ? 100 : clock.percent;

  // the room is what pays for the start: while it runs, the page stays
  useHoldPage(remaining > 0);

  const doLeave = () => {
    if (left.current) return;
    left.current = true;

    leaveWaitingRoom(server.uuid, blocked === true)
      .then(() => leavePage(`/server/${server.uuidShort}`))
      .catch((err) => {
        left.current = false;
        if (err instanceof AxiosError && err.response?.status === 409) {
          refresh();
          return;
        }
        addToast(httpErrorToHuman(err), 'error');
      });
  };

  // through by itself once the room may let the user go
  useEffect(() => {
    if (room.ready) doLeave();
  }, [room.ready]);

  return (
    <div className='theme-wait-page'>
      <section className='theme-eco-card'>
        <div className='theme-wait-head'>
          <div className='flex min-w-0 flex-col gap-1'>
            <div className='theme-wait-title'>
              <Spinner size={20} />
              <h3 className='theme-eco-heading truncate'>{t(`${page}.heading`, {})}</h3>
            </div>
            <p className='theme-row-body'>{t(`${page}.body`, { seconds: room.maxSeconds })}</p>
          </div>

          <div className='theme-wait-figure'>
            <span className='theme-caption'>{t(`${page}.waitLabel`, {})}</span>
            <span className='theme-figure theme-figure-accent'>
              {remaining > 0 ? t(`${page}.seconds`, { seconds: remaining }) : t(`${page}.ready`, {})}
            </span>
          </div>
        </div>

        <div className='theme-wait-track'>
          <div className='theme-wait-fill' style={{ width: `${percent}%` }} />
        </div>
      </section>

      <AdStack part='above' />
      <AdStack part='below' />
    </div>
  );
}

export default function ServerWaitingRoom() {
  const { t } = useTranslations();
  const leavePage = useLeaveWaitingPage();
  const server = useServerStore((state) => state.server);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).waitingRoom(),
    queryFn: () => getWaitingRoom(server.uuid),
  });
  const status = resource.data?.status;

  useEffect(() => {
    if (status !== 'waiting') return;

    const interval = setInterval(() => resource.refetch(), 2_000);
    return () => clearInterval(interval);
  }, [status]);

  // nothing to wait for (it was let through already, or never waited): the console
  useEffect(() => {
    if (status === 'none' || status === 'released') leavePage(`/server/${server.uuidShort}`);
  }, [status]);

  return (
    <ServerContentContainer title={t(`${page}.title`, {})} subtitle={t(`${page}.subtitle`, { server: server.name })}>
      <ResourceView resource={resource}>
        {(room) =>
          room.status === 'waiting' ? (
            <Waiting room={room} refetch={resource.refetch} refresh={resource.invalidate} />
          ) : null
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
