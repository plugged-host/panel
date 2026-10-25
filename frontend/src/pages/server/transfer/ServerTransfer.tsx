// plugged.host: a player moving their server to another location, laid out like the Pelican panel's
// Transfer page: while a move runs its progress and activity, otherwise the picker; then the
// earlier moves (docs/features/transfers.md)
import { useEffect, useRef } from 'react';
import getServerTransfer from '@/api/server/transfer/getServerTransfer.ts';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { isActiveTransfer } from '@/lib/domain/transfer.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import TransferIdle from './TransferIdle.tsx';
import { TransferHistory, TransferTimeline } from './TransferParts.tsx';
import TransferProgress from './TransferProgress.tsx';

export default function ServerTransfer() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).transfer(),
    queryFn: () => getServerTransfer(server.uuid),
    keepPrevious: true,
  });
  const active = isActiveTransfer(resource.data?.request);

  // the Pelican panel polls every two seconds while a move runs
  useEffect(() => {
    if (!active) return;

    const interval = setInterval(() => resource.refetch(), 2_000);
    return () => clearInterval(interval);
  }, [active]);

  // a move an admin starts, or one that ends, shows up here at once
  const transferring = useRef(server.isTransferring);
  useEffect(() => {
    if (transferring.current === server.isTransferring) return;

    transferring.current = server.isTransferring;
    resource.refetch();
  }, [server.isTransferring]);

  return (
    <ServerContentContainer
      title={t('pages.server.transfer.title', {})}
      subtitle={t('pages.server.transfer.subtitle', {})}
    >
      <ResourceView resource={resource}>
        {(options) => (
          <div className='flex flex-col gap-6'>
            {isActiveTransfer(options.request) ? (
              <>
                <TransferProgress options={options} request={options.request} refresh={resource.invalidate} />
                <TransferTimeline events={options.events} />
              </>
            ) : (
              <>
                <TransferIdle options={options} refresh={resource.invalidate} />
                {options.request && <TransferTimeline events={options.events} />}
              </>
            )}
            <TransferHistory history={options.history} />
          </div>
        )}
      </ResourceView>
    </ServerContentContainer>
  );
}
