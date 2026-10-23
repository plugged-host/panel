// plugged.host: a start waiting for room on a busy node (docs/features/start-queue.md)
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getStartQueue from '@/api/server/queue/getStartQueue.ts';
import leaveStartQueue from '@/api/server/queue/leaveStartQueue.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { formatSeconds } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverStartQueueSchema } from '@/lib/schemas/server/queue.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Queue = z.infer<typeof serverStartQueueSchema>;

function Waiting({ queue, refresh }: { queue: Queue; refresh: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const economy = useGlobalStore((state) => state.settings.economy);

  const [leaving, setLeaving] = useState(false);

  const doLeave = () => {
    setLeaving(true);

    leaveStartQueue(server.uuid)
      .then(() => {
        addToast(t('pages.server.queue.toast.left', {}), 'success');
        navigate(`/server/${server.uuidShort}`);
      })
      .catch((err) => {
        addToast(httpErrorToHuman(err), 'error');
        refresh();
      })
      .finally(() => setLeaving(false));
  };

  return (
    <Stack gap='lg'>
      <Text size='sm' c='dimmed'>
        {t('pages.server.queue.description', {})}
      </Text>

      <Stack gap={4}>
        <Title order={2}>{t('pages.server.queue.position', { position: queue.position })}</Title>
        <Text size='sm' c='dimmed'>
          {t('pages.server.queue.waited', { waited: formatSeconds(queue.waited) })}{' '}
          {t('pages.server.queue.remaining', { remaining: formatSeconds(queue.remaining) })}
        </Text>
      </Stack>

      {queue.membershipOffer && (
        <Stack gap='xs' align='flex-start'>
          <Text size='sm'>
            {t('pages.server.queue.membership', {
              name: (queue.priority > 0 ? economy?.plusplusName : economy?.plusName) ?? '',
            })}
          </Text>
          <Button onClick={() => navigate('/account/membership')}>
            {t('pages.server.queue.button.membership', {
              name: (queue.priority > 0 ? economy?.plusplusName : economy?.plusName) ?? '',
            })}
          </Button>
        </Stack>
      )}

      {queue.moveOffer && (
        <Stack gap='xs' align='flex-start'>
          <Text size='sm'>{t('pages.server.queue.move', {})}</Text>
          <Button color='gray' onClick={() => navigate(`/server/${server.uuidShort}/transfer`)}>
            {t('pages.server.queue.button.move', {})}
          </Button>
        </Stack>
      )}

      <Group>
        <Button color='gray' loading={leaving} onClick={doLeave}>
          {t('pages.server.queue.button.leave', {})}
        </Button>
      </Group>
    </Stack>
  );
}

export default function ServerQueue() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
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
    if (status !== 'released') return;

    addToast(t('pages.server.queue.toast.released', {}), 'success');
    navigate(`/server/${server.uuidShort}`, { replace: true });
  }, [status]);

  return (
    <ServerContentContainer title={t('pages.server.queue.title', {})}>
      <ResourceView resource={resource}>
        {(queue) =>
          queue.status === 'waiting' ? (
            <Waiting queue={queue} refresh={resource.invalidate} />
          ) : (
            <Stack gap='xs' align='flex-start'>
              <Text c='dimmed'>{t('pages.server.queue.notWaiting', {})}</Text>
              <Button color='gray' onClick={() => navigate(`/server/${server.uuidShort}`)}>
                {t('pages.server.queue.button.console', {})}
              </Button>
            </Stack>
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
