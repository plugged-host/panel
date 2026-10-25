// plugged.host: the strip on every server page while the server moves, and for two minutes after,
// like the Pelican panel's TransferStrip: a status dot, the state and where to, a thin bar, the
// percent, and a link to the Transfer page (docs/features/transfers.md)
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { NavLink, useLocation } from 'react-router';
import { z } from 'zod';
import getServerTransferStatus from '@/api/server/transfer/getServerTransferStatus.ts';
import { isInLimbo } from '@/lib/domain/limbo.ts';
import { isActiveTransfer, transferStateText } from '@/lib/domain/transfer.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverTransferStatusSchema } from '@/lib/schemas/server/transfer.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Status = z.infer<typeof serverTransferStatusSchema>;

/**
 * The move the strip shows (`status`, null when there is none), and whether the strip or the
 * Transfer page covers this server's moves here (`covered`): then the upstream "transferring"
 * banner stays away. It shows only for a move neither covers, like an admin's own transfer while
 * moving is off or for a subuser without the permission.
 */
export function useTransferStrip(): { status: Status | null; covered: boolean } {
  const server = useServerStore((state) => state.server);
  const transferEnabled = useGlobalStore((state) => !!state.settings.games?.transferEnabled);
  const canTransfer = useServerCan('settings.transfer');
  const location = useLocation();

  const onTransferPage = [server.uuid, server.uuidShort].some((id) =>
    location.pathname.startsWith(`/server/${id}/transfer`),
  );
  const enabled = transferEnabled && canTransfer && !isInLimbo(server.limboState);

  const query = useQuery({
    queryKey: [...queryKeys.server(server.uuid).transferStatus(), server.isTransferring],
    queryFn: () => getServerTransferStatus(server.uuid),
    enabled,
    placeholderData: keepPreviousData,
    staleTime: 15_000,
    // fast while it runs, slowly while it shows how it ended so it clears itself afterwards
    refetchInterval: (current) =>
      isActiveTransfer(current.state.data?.request) ? 3_000 : current.state.data ? 15_000 : false,
  });

  if (!enabled || query.isError) return { status: null, covered: false };
  if (onTransferPage) return { status: null, covered: true };

  // a move being asked for (no answer yet, or one kept from before the server's state changed) counts
  return { status: query.data ?? null, covered: query.data !== null || query.isPlaceholderData };
}

export default function TransferStrip({ status }: { status: Status }) {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const active = isActiveTransfer(status.request);
  const ok = status.request.status === 'completed';
  const tone = active ? 'run' : ok ? 'ok' : 'bad';
  const state = status.request.status === 'cancelling' ? 'cancelling' : active ? status.progress?.state : null;
  const percent = ok ? 100 : active ? Math.min(100, Math.max(0, status.progress?.percent ?? 0)) : 100;

  return (
    <div className='theme-main theme-main-banner'>
      <NavLink to={`/server/${server.uuidShort}/transfer`} className='theme-transfer-strip' data-tone={tone}>
        <i className='theme-transfer-dot' aria-hidden />
        <span className='theme-transfer-strip-text'>
          <strong>{transferStateText(t, state ?? status.request.status)}</strong>
          <small>{status.request.toLocation?.name ?? t('pages.server.transfer.unknownLocation', {})}</small>
        </span>
        <span className='theme-transfer-strip-bar'>
          <span style={{ width: `${percent}%` }} />
        </span>
        {active && <span className='theme-transfer-strip-pct font-mono'>{percent}%</span>}
        <span className='theme-transfer-strip-open'>{t('common.button.details', {})}</span>
      </NavLink>
    </div>
  );
}
