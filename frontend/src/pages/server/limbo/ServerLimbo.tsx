// plugged.host: a server limbo parked (or is parking, or brings back), laid out like the Pelican
// panel's restore page: one card with what happened and what a restore costs, Restore in the
// header, and the locations when its own has no room (docs/features/limbo.md)
import { useEffect, useState } from 'react';
import { Navigate, useNavigate } from 'react-router';
import { z } from 'zod';
import getServerLimbo from '@/api/server/limbo/getServerLimbo.ts';
import restoreServer from '@/api/server/limbo/restoreServer.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import LocationList from '@/elements/LocationList.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { isInLimbo } from '@/lib/domain/limbo.ts';
import { tierName } from '@/lib/economy.ts';
import { formatTimestamp } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverLimboSchema } from '@/lib/schemas/server/limbo.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { usePowerAction } from '@/plugins/server/usePowerAction.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import UpgradeOfferCallout from '../upgrades/UpgradeOfferCallout.tsx'; // plugged.host

type Limbo = z.infer<typeof serverLimboSchema>;

const page = 'pages.server.limbo';

const moving = (state: Limbo['state']) => state === 'parking' || state === 'restoring' || state === 'unpacking';

function Parked({
  limbo,
  locationUuid,
  setLocationUuid,
}: {
  limbo: Limbo;
  locationUuid: string | null;
  setLocationUuid: (uuid: string) => void;
}) {
  const { t } = useTranslations();
  // the locations it may come back in: the owner's, with room
  const choices = limbo.locations.filter((location) => location.unlocked && location.available);

  return (
    <div className='flex flex-col gap-6'>
      <section className='theme-eco-card theme-card-loose'>
        <div className='flex flex-col gap-1'>
          <p className='theme-row-body'>{t(`${page}.body`, {})}</p>
          {limbo.parked && (
            <p className='theme-row-body'>{t(`${page}.entered`, { time: formatTimestamp(limbo.parked) })}</p>
          )}
          {limbo.deleteAt && (
            <p className='theme-row-body'>{t(`${page}.deletesAt`, { time: formatTimestamp(limbo.deleteAt) })}</p>
          )}
          {limbo.error && (
            <p className='theme-row-body theme-row-error'>{t(`${page}.failed`, { error: limbo.error })}</p>
          )}
        </div>

        {limbo.tokens !== null && (
          <dl className='theme-figures'>
            <div>
              <dt className='theme-caption'>{t(`${page}.costLabel`, {})}</dt>
              <dd className='theme-figure font-mono'>{t(`${page}.tokens`, { count: limbo.cost })}</dd>
            </div>
            <div>
              <dt className='theme-caption'>{t(`${page}.balanceLabel`, {})}</dt>
              <dd className='theme-figure font-mono'>{t(`${page}.tokens`, { count: limbo.tokens })}</dd>
            </div>
          </dl>
        )}
      </section>

      {!limbo.homeAvailable && (
        <section className='flex flex-col gap-3'>
          <div className='flex flex-col gap-1'>
            <h3 className='theme-eco-heading'>{t(`${page}.pick.title`, {})}</h3>
            <p className='theme-row-body'>
              {choices.length > 0 ? t(`${page}.pick.body`, {}) : t(`${page}.pick.none`, {})}
            </p>
          </div>

          {choices.length > 0 && (
            <LocationList
              locations={choices}
              selected={locationUuid}
              onSelect={setLocationUuid}
              badges={(location) =>
                location.tier && location.tier !== 'free' ? (
                  <Badge color='gray' size='sm'>
                    {tierName(location.tier)}
                  </Badge>
                ) : null
              }
            />
          )}
        </section>
      )}

      {/* a keep-awake pass keeps it from being deleted, and from being parked again once it is back
          (docs/features/server-upgrades.md) */}
      <UpgradeOfferCallout kind='keep_awake' text='keepAwakeParked' show className='' />
    </div>
  );
}

export default function ServerLimbo() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const powerAction = usePowerAction();

  const [locationUuid, setLocationUuid] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [keeping, setKeeping] = useState(false);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).limbo(),
    queryFn: () => getServerLimbo(server.uuid),
  });
  const limbo = resource.data;
  const state = limbo?.state ?? null;
  const needsLocation = !!limbo && !limbo.homeAvailable;

  // parks and restores run on the panel: follow them
  useEffect(() => {
    if (!moving(state)) return;

    const interval = setInterval(() => resource.refetch(), 5_000);
    return () => clearInterval(interval);
  }, [state]);

  // back to an ordinary server: the rest of its pages open again, at the console
  useEffect(() => {
    if (!limbo || limbo.state === server.limboState) return;

    updateServer({ limboState: limbo.state });
    if (!isInLimbo(limbo.state) && isInLimbo(server.limboState)) {
      navigate(`/server/${server.uuidShort}`, { replace: true });
    }
  }, [limbo?.state]);

  const doRestore = async () => {
    try {
      await restoreServer(server.uuid, needsLocation ? locationUuid : null);
      addToast(t(`${page}.toast.restoring`, {}), 'success');
      setConfirming(false);
      resource.invalidate();
    } catch {
      setConfirming(false);

      // why, from the server's state now: short of tokens, or no room where it was to go
      const fresh = (await resource.refetch()).data ?? limbo;
      if (fresh && fresh.tokens !== null && fresh.tokens < fresh.cost) {
        addToast(t(`${page}.toast.insufficient`, { cost: fresh.cost }), 'error');
      } else if (fresh && !fresh.homeAvailable) {
        setLocationUuid(null);
        addToast(t(`${page}.toast.locationFull`, {}), 'error');
      } else {
        addToast(t(`${page}.toast.failed`, {}), 'error');
      }
    }
  };

  const doKeep = () => {
    setKeeping(true);
    powerAction('start')
      .then(() => resource.invalidate())
      .finally(() => setKeeping(false));
  };

  return (
    <ServerContentContainer
      title={t(`${page}.title`, {})}
      contentRight={
        state === 'parked' ? (
          <Button disabled={needsLocation && !locationUuid} onClick={() => setConfirming(true)}>
            {t(`${page}.button.restore`, {})}
          </Button>
        ) : state === 'parking' ? (
          <Button loading={keeping} onClick={doKeep}>
            {t(`${page}.button.keep`, {})}
          </Button>
        ) : undefined
      }
    >
      {limbo && (
        <ConfirmationModal
          opened={confirming}
          onClose={() => setConfirming(false)}
          title={t(`${page}.button.restore`, {})}
          confirm={t(`${page}.modal.confirm`, {})}
          confirmColor='blue'
          onConfirmed={doRestore}
        >
          {limbo.cost > 0 ? t(`${page}.modal.content`, { cost: limbo.cost }) : t(`${page}.modal.contentFree`, {})}
        </ConfirmationModal>
      )}

      <ResourceView resource={resource}>
        {(view) =>
          view.state === 'parked' ? (
            <Parked limbo={view} locationUuid={locationUuid} setLocationUuid={setLocationUuid} />
          ) : view.state === 'parking' ? (
            <section className='theme-eco-card'>
              <p className='theme-row-body'>{t(`${page}.parking`, {})}</p>
            </section>
          ) : view.state === 'restoring' || view.state === 'unpacking' ? (
            <section className='theme-eco-card'>
              <p className='theme-row-body'>{t(`${page}.restoring`, {})}</p>
            </section>
          ) : (
            // an ordinary server: its console
            <Navigate to={`/server/${server.uuidShort}`} replace />
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
