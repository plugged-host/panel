// plugged.host: a player moving their server to another location (docs/features/transfers.md)
import { ReactNode, useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import cancelServerTransfer from '@/api/server/transfer/cancelServerTransfer.ts';
import getServerTransfer from '@/api/server/transfer/getServerTransfer.ts';
import requestServerTransfer from '@/api/server/transfer/requestServerTransfer.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { tierName } from '@/lib/economy.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { activeTransferStatuses, serverTransferOptionsSchema } from '@/lib/schemas/server/transfer.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Options = z.infer<typeof serverTransferOptionsSchema>;
type Request = NonNullable<Options['request']>;

const isActive = (request: Request | null) =>
  !!request && (activeTransferStatuses as readonly string[]).includes(request.status);

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Stack gap='xs'>
      <Title order={3} fz='md' fw={600}>
        {title}
      </Title>
      {children}
    </Stack>
  );
}

function RequestStatus({
  request,
  onCancel,
  cancelling,
}: {
  request: Request;
  onCancel: () => void;
  cancelling: boolean;
}) {
  const { t } = useTranslations();
  const base = 'pages.server.transfer.status';

  if (isActive(request)) {
    return (
      <Stack gap='xs' align='flex-start'>
        <Text size='sm'>
          {request.status === 'queued'
            ? t(`${base}.queued`, {})
            : request.status === 'cancelling'
              ? t(`${base}.cancelling`, {})
              : t(`${base}.running`, { attempt: request.attempts })}
        </Text>
        {request.origin === 'admin' ? (
          <Text size='sm' c='dimmed'>
            {t(`${base}.byAdmin`, {})}
          </Text>
        ) : (
          request.status !== 'cancelling' && (
            <Button color='gray' loading={cancelling} onClick={onCancel}>
              {t('pages.server.transfer.button.cancel', {})}
            </Button>
          )
        )}
      </Stack>
    );
  }

  return (
    <Stack gap={0}>
      <Text size='sm' c={request.status === 'failed' ? 'red' : undefined}>
        {request.status === 'completed'
          ? t(`${base}.completed`, {})
          : request.status === 'failed'
            ? t(`${base}.failed`, { error: request.error ?? '' })
            : t(`${base}.cancelled`, {})}
      </Text>
      {request.finished && (
        <Text size='xs' c='dimmed'>
          <FormattedTimestamp timestamp={request.finished} />
        </Text>
      )}
    </Stack>
  );
}

function TransferForm({ options, refresh }: { options: Options; refresh: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const { user } = useAuth();
  const server = useServerStore((state) => state.server);

  const [locationUuid, setLocationUuid] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  const active = isActive(options.request);
  const coolingDown = !!options.cooldownUntil && !user?.admin;
  const location = options.locations.find((location) => location.uuid === locationUuid) ?? null;

  if (!options.enabled && !user?.admin) {
    return <Text c='dimmed'>{t('pages.server.transfer.disabled', {})}</Text>;
  }

  const doRequest = async () => {
    if (!locationUuid) return;

    await requestServerTransfer(server.uuid, { locationUuid })
      .then(() => {
        addToast(t('pages.server.transfer.toast.requested', {}), 'success');
        setConfirming(false);
        setLocationUuid(null);
      })
      .catch((err) => {
        addToast(httpErrorToHuman(err), 'error');
        setConfirming(false);
      })
      .finally(refresh);
  };

  const doCancel = () => {
    setCancelling(true);

    cancelServerTransfer(server.uuid)
      .then(() => addToast(t('pages.server.transfer.toast.cancelled', {}), 'success'))
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => {
        setCancelling(false);
        refresh();
      });
  };

  return (
    <Stack gap='lg'>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t('pages.server.transfer.modal.title', {})}
        confirm={t('pages.server.transfer.button.move', {})}
        onConfirmed={doRequest}
      >
        {t('pages.server.transfer.modal.content', { location: location?.name ?? '' }).md()}
      </ConfirmationModal>

      <Text size='sm' c='dimmed'>
        {t('pages.server.transfer.description', {})}
      </Text>

      {options.request && (
        <Section title={t(active ? 'pages.server.transfer.currentMove' : 'pages.server.transfer.lastMove', {})}>
          <RequestStatus request={options.request} onCancel={doCancel} cancelling={cancelling} />
        </Section>
      )}

      <Section title={t('pages.server.transfer.location', {})}>
        <div className='theme-choice-grid'>
          {options.locations.map((location) => {
            const current = location.uuid === options.currentLocationUuid;

            return (
              <button
                key={location.uuid}
                type='button'
                className='theme-choice'
                data-selected={location.uuid === locationUuid || undefined}
                aria-pressed={location.uuid === locationUuid}
                disabled={active || coolingDown || !location.unlocked || !location.available}
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
                      : current
                        ? t('pages.server.transfer.currentLocation', {})
                        : location.latency !== null
                          ? t('pages.account.createServer.latency', { ms: location.latency })
                          : (location.description ?? '')}
                </span>
              </button>
            );
          })}
        </div>
      </Section>

      {coolingDown && options.cooldownUntil && (
        <Stack gap={0}>
          <Text size='sm'>{t('pages.server.transfer.cooldown', {})}</Text>
          <Text size='xs' c='dimmed'>
            {t('pages.server.transfer.cooldownUntil', {})} <FormattedTimestamp timestamp={options.cooldownUntil} />
          </Text>
        </Stack>
      )}

      <Group>
        <Button disabled={!location || active || coolingDown} onClick={() => setConfirming(true)}>
          {t('pages.server.transfer.button.move', {})}
        </Button>
      </Group>
    </Stack>
  );
}

export default function ServerTransfer() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).transfer(),
    queryFn: () => getServerTransfer(server.uuid),
  });
  const active = isActive(resource.data?.request ?? null);

  // a move settles in the background: follow it while it runs
  useEffect(() => {
    if (!active) return;

    const interval = setInterval(() => resource.refetch(), 10_000);
    return () => clearInterval(interval);
  }, [active]);

  return (
    <ServerContentContainer title={t('pages.server.transfer.title', {})}>
      <ResourceView resource={resource}>
        {(options) => <TransferForm options={options} refresh={resource.invalidate} />}
      </ResourceView>
    </ServerContentContainer>
  );
}
