// plugged.host: a server limbo stopped or parked, and its restore (docs/features/limbo.md)
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getServerLimbo from '@/api/server/limbo/getServerLimbo.ts';
import restoreServer from '@/api/server/limbo/restoreServer.ts';
import { AdStack } from '@/elements/ads/AdSlot.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { tierName } from '@/lib/economy.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverLimboSchema } from '@/lib/schemas/server/limbo.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { usePowerAction } from '@/plugins/server/usePowerAction.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Limbo = z.infer<typeof serverLimboSchema>;

const moving = (state: Limbo['state']) => state === 'parking' || state === 'restoring' || state === 'unpacking';

function Parked({ limbo, refresh }: { limbo: Limbo; refresh: (limbo?: Limbo) => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);

  const [locationUuid, setLocationUuid] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(false);

  const short = limbo.tokens !== null && limbo.tokens < limbo.cost;
  const needsLocation = !limbo.homeAvailable;

  const doRestore = () => {
    setRestoring(true);

    restoreServer(server.uuid, needsLocation ? locationUuid : null)
      .then((next) => {
        addToast(t('pages.server.limbo.toast.restoring', {}), 'success');
        refresh(next);
      })
      .catch((err) => {
        addToast(httpErrorToHuman(err), 'error');
        refresh();
      })
      .finally(() => setRestoring(false));
  };

  return (
    <Stack gap='lg'>
      <Stack gap={4}>
        <Text>{t('pages.server.limbo.parked', {})}</Text>
        {limbo.lastActivity && (
          <Text size='sm' c='dimmed'>
            {t('pages.server.limbo.lastActivity', {})} <FormattedTimestamp timestamp={limbo.lastActivity} />
          </Text>
        )}
      </Stack>

      {limbo.error && (
        <Text size='sm' c='red'>
          {t('pages.server.limbo.failed', { error: limbo.error })}
        </Text>
      )}

      {needsLocation ? (
        <Stack gap='xs'>
          <Title order={3} fz='md' fw={600}>
            {t('pages.server.limbo.location', {})}
          </Title>
          <Text size='sm' c='dimmed'>
            {t('pages.server.limbo.homeFull', {})}
          </Text>
          <div className='theme-choice-grid'>
            {limbo.locations.map((location) => (
              <button
                key={location.uuid}
                type='button'
                className='theme-choice'
                data-selected={location.uuid === locationUuid || undefined}
                aria-pressed={location.uuid === locationUuid}
                disabled={!location.unlocked || !location.available}
                onClick={() => setLocationUuid(location.uuid)}
              >
                <span className='theme-choice-title'>
                  {location.flag && (
                    <img src={`/flags/${location.flag}.svg`} alt='' className='w-5 h-5 rounded-md shrink-0' />
                  )}
                  {location.name}
                </span>
                <span className='theme-choice-text'>
                  {!location.unlocked
                    ? t('pages.account.createServer.locationLocked', { tier: tierName(location.tier) })
                    : !location.available
                      ? t('pages.account.createServer.locationFull', {})
                      : location.uuid === limbo.homeLocationUuid
                        ? t('pages.server.transfer.currentLocation', {})
                        : location.latency !== null
                          ? t('pages.account.createServer.latency', { ms: location.latency })
                          : (location.description ?? '')}
                </span>
              </button>
            ))}
          </div>
        </Stack>
      ) : (
        <Text size='sm' c='dimmed'>
          {t('pages.server.limbo.home', {})}
        </Text>
      )}

      {limbo.cost > 0 && (
        <Stack gap={4}>
          <Text size='sm'>{t('pages.server.limbo.cost', { cost: limbo.cost, tokens: limbo.tokens ?? 0 })}</Text>
          {short && (
            <Group>
              <Button color='gray' onClick={() => navigate('/account/tokens')}>
                {t('pages.server.limbo.button.tokens', {})}
              </Button>
            </Group>
          )}
        </Stack>
      )}

      <Group>
        <Button loading={restoring} disabled={short || (needsLocation && !locationUuid)} onClick={doRestore}>
          {limbo.cost > 0
            ? t('pages.server.limbo.button.restoreFor', { cost: limbo.cost })
            : t('pages.server.limbo.button.restore', {})}
        </Button>
      </Group>
    </Stack>
  );
}

export default function ServerLimbo() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const powerAction = usePowerAction();

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).limbo(),
    queryFn: () => getServerLimbo(server.uuid),
  });
  const state = resource.data?.state;

  // parks and restores run on the panel: follow them
  useEffect(() => {
    if (!moving(state ?? null)) return;

    const interval = setInterval(() => resource.refetch(), 5_000);
    return () => clearInterval(interval);
  }, [state]);

  // back to an ordinary server: the rest of its pages open again
  useEffect(() => {
    if (!resource.data || resource.data.state === server.limboState) return;

    const back = !resource.data.state || resource.data.state === 'stopped';
    updateServer({ limboState: resource.data.state });
    if (back && server.limboState && server.limboState !== 'stopped') {
      addToast(t('pages.server.limbo.toast.restored', {}), 'success');
      navigate(`/server/${server.uuidShort}`, { replace: true });
    }
  }, [resource.data?.state]);

  return (
    <ServerContentContainer title={t('pages.server.limbo.title', {})}>
      <ResourceView resource={resource}>
        {(limbo) =>
          limbo.state === 'parked' ? (
            <Parked
              limbo={limbo}
              refresh={() => {
                resource.invalidate();
              }}
            />
          ) : limbo.state === 'parking' ? (
            <Stack gap='xs' align='flex-start'>
              <Text>{t('pages.server.limbo.parking', {})}</Text>
              <Button onClick={() => powerAction('start').then(() => resource.invalidate())}>
                {t('pages.server.limbo.button.keep', {})}
              </Button>
            </Stack>
          ) : limbo.state === 'restoring' || limbo.state === 'unpacking' ? (
            <Stack gap='lg'>
              <Group gap='sm'>
                <Spinner size={20} />
                <Text>
                  {limbo.state === 'restoring'
                    ? t('pages.server.limbo.restoring', {})
                    : t('pages.server.limbo.unpacking', {})}
                </Text>
              </Group>
              {/* a restore takes a while: the waiting rooms' ads (docs/features/ads.md) */}
              <AdStack />
            </Stack>
          ) : (
            <Stack gap='xs' align='flex-start'>
              <Text c='dimmed'>{t('pages.server.limbo.notParked', {})}</Text>
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
