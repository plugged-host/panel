// plugged.host: a start in the waiting room (docs/features/ads.md#waiting-room)
import { AxiosError } from 'axios';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getWaitingRoom from '@/api/server/waiting/getWaitingRoom.ts';
import leaveWaitingRoom from '@/api/server/waiting/leaveWaitingRoom.ts';
import { useAdBlocked } from '@/elements/ads/AdBlockNotice.tsx';
import { adScriptsLoaded } from '@/elements/ads/AdScripts.tsx';
import { AdStack } from '@/elements/ads/AdSlot.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Progress from '@/elements/feedback/Progress.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { formatSeconds } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverWaitingRoomSchema } from '@/lib/schemas/server/waitingRoom.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Room = z.infer<typeof serverWaitingRoomSchema>;

const page = 'pages.server.waitingRoom';

/** To the console; a full load when page-level ad scripts ran here, so they end with the room. */
function useToConsole() {
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);

  return () => {
    const path = `/server/${server.uuidShort}`;
    if (adScriptsLoaded()) {
      window.location.assign(path);
    } else {
      navigate(path, { replace: true });
    }
  };
}

function Waiting({ room, refresh }: { room: Room; refresh: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const toConsole = useToConsole();
  const server = useServerStore((state) => state.server);
  const economy = useGlobalStore((state) => state.settings.economy);
  const blocked = useAdBlocked('server');

  const [leaving, setLeaving] = useState(false);
  const left = useRef(false);

  const doLeave = () => {
    if (left.current) return;
    left.current = true;
    setLeaving(true);

    leaveWaitingRoom(server.uuid, blocked === true)
      .then(() => toConsole())
      .catch((err) => {
        left.current = false;
        if (err instanceof AxiosError && err.response?.status === 409) {
          refresh();
          return;
        }
        addToast(httpErrorToHuman(err), 'error');
      })
      .finally(() => setLeaving(false));
  };

  // through by itself once the room may let the user go
  useEffect(() => {
    if (room.ready) doLeave();
  }, [room.ready]);

  const remaining = Math.max(0, (room.running ? room.minSeconds : room.maxSeconds) - room.elapsed);

  return (
    <Stack gap='lg'>
      <Text size='sm' c='dimmed'>
        {t(`${page}.description`, {})}
      </Text>

      <Stack gap='xs'>
        <Text size='sm'>
          {room.ready
            ? t(`${page}.ready`, {})
            : room.running
              ? t(`${page}.running`, { remaining: formatSeconds(remaining) })
              : t(`${page}.starting`, { remaining: formatSeconds(remaining) })}
        </Text>
        <Progress
          value={(room.elapsed / Math.max(1, room.running ? room.minSeconds : room.maxSeconds)) * 100}
          hourglass={false}
          withLabel={false}
        />
      </Stack>

      <Group>
        <Button disabled={!room.ready} loading={leaving} onClick={doLeave}>
          {t(`${page}.button.continue`, {})}
        </Button>
        {economy?.membershipEnabled && (
          <Button color='gray' onClick={() => navigate('/account/membership')}>
            {t(`${page}.button.membership`, { name: economy.plusplusName })}
          </Button>
        )}
      </Group>

      <AdStack />
    </Stack>
  );
}

export default function ServerWaitingRoom() {
  const { t } = useTranslations();
  const toConsole = useToConsole();
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

  return (
    <ServerContentContainer title={t(`${page}.title`, {})}>
      <ResourceView resource={resource}>
        {(room) =>
          room.status === 'waiting' ? (
            <Waiting room={room} refresh={resource.invalidate} />
          ) : (
            <Stack gap='xs' align='flex-start'>
              <Text c='dimmed'>{t(`${page}.notWaiting`, {})}</Text>
              <Button color='gray' onClick={toConsole}>
                {t(`${page}.button.console`, {})}
              </Button>
            </Stack>
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
