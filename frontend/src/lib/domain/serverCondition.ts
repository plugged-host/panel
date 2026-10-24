// plugged.host: a server's condition as the Pelican panel shows it (Server::condition with its label and colour), for the server cards and the console's Status block
import { z } from 'zod';
import { isInLimbo } from '@/lib/domain/limbo.ts';
import { serverStatusInfo } from '@/lib/domain/server.ts';
import { serverPowerState, serverSchema } from '@/lib/schemas/server/server.ts';
import { getTranslations } from '@/providers/TranslationProvider.tsx';

/** Filament's colour names, as `fi-color-*` has them; the status bar paints them (app.css). */
export type ServerConditionColor = 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'gray';

export interface ServerCondition {
  /** null while the power state is not known yet */
  label: string | null;
  color: ServerConditionColor;
  /** the label is the live power state, so its uptime may follow it */
  live: boolean;
}

const powerStateColors: Record<z.infer<typeof serverPowerState>, ServerConditionColor> = {
  running: 'success',
  starting: 'warning',
  stopping: 'warning',
  offline: 'danger',
};

/**
 * What the server is doing, the most important first: a suspension, limbo, a transfer, node
 * maintenance and the server's own status (installing, restoring) come before the container's
 * power state, which is only known once its stats arrive (`state`). Colours follow Pelican's
 * ServerState and ContainerStatus enums.
 */
export function serverCondition(
  server: Pick<
    z.infer<typeof serverSchema>,
    'isSuspended' | 'limboState' | 'isTransferring' | 'nodeMaintenanceEnabled' | 'status'
  >,
  state: z.infer<typeof serverPowerState> | null | undefined,
  statsUnavailable = false,
): ServerCondition {
  const { t } = getTranslations();

  if (server.isSuspended) {
    return { label: t('common.server.state.suspended', {}), color: 'warning', live: false };
  }

  if (isInLimbo(server.limboState)) {
    const limbo = server.limboState === 'parking' || server.limboState === 'parked' ? server.limboState : 'restoring';

    return {
      label: t(`common.server.state.${limbo}`, {}),
      color: limbo === 'parked' ? 'gray' : 'primary',
      live: false,
    };
  }

  if (server.isTransferring) {
    return { label: t('common.server.state.transferring', {}), color: 'primary', live: false };
  }

  if (server.nodeMaintenanceEnabled) {
    return { label: t('common.server.state.nodeMaintenance', {}), color: 'warning', live: false };
  }

  if (server.status) {
    const info = serverStatusInfo[server.status];

    return { label: info.label(), color: info.failed ? 'danger' : 'primary', live: false };
  }

  if (state) {
    return { label: t(`common.enum.serverState.${state}`, {}), color: powerStateColors[state], live: true };
  }

  if (statsUnavailable) {
    return { label: t('common.server.state.resourcesUnavailable', {}), color: 'danger', live: false };
  }

  return { label: null, color: 'gray', live: false };
}
