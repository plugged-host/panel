import {
  faAdd,
  faEllipsisVertical,
  faInfoCircle,
  faMinus,
  faPlay,
  faRotateRight,
  faSkull,
  faStop,
  faTriangleExclamation,
  faUsers,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import classNames from 'classnames';
import { useCallback, useMemo, useState } from 'react';
import { NavLink } from 'react-router';
import { z } from 'zod';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ContextMenu, { ContextMenuItem } from '@/elements/overlays/ContextMenu.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ScrollingText from '@/elements/ScrollingText.tsx';
import RedactedText from '@/elements/typography/RedactedText.tsx';
import { formatAllocation, serverStatusInfo, statusToColor } from '@/lib/domain/server.ts';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { serverPowerAction, serverSchema } from '@/lib/schemas/server/server.ts';
import { useBulkPowerActions } from '@/plugins/server/useBulkPowerActions.ts';
import { useServerListShowOthers } from '@/plugins/server/useServerListShowOthers.ts';
import { useServerStats, useServerStatsUnavailable } from '@/plugins/server/useServerStats.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useUserStore } from '@/stores/user.ts';
import ServerAddGroupModal from './modals/ServerAddGroupModal.tsx';

export default function ServerItem({
  server,
  to,
  showGroupAddButton = false,
  showForeignServerBadge = false,
  showContextMenu = false,
  onGroupRemove,
  isSelected = false,
  onSelectionChange,
  onClick,
  showSelection = true,
  sKeyPressedRef,
}: {
  server: z.infer<typeof serverSchema>;
  to?: string;
  showGroupAddButton?: boolean;
  showForeignServerBadge?: boolean;
  showContextMenu?: boolean;
  onGroupRemove?: () => void;
  isSelected?: boolean;
  onSelectionChange?: (selected: boolean) => void;
  onClick?: (event: React.MouseEvent) => void;
  showSelection?: boolean;
  sKeyPressedRef?: React.RefObject<boolean>;
}) {
  const { t } = useTranslations();
  const { user } = useAuth();
  const serverGroups = useUserStore((state) => state.serverGroups);
  const [serverListShowOthers] = useServerListShowOthers();

  const [openModal, setOpenModal] = useState<'add-group' | 'kill' | null>(null);
  const stats = useServerStats(server);
  const statsUnavailable = useServerStatsUnavailable(server);

  const availableServerGroups = useMemo(
    () => serverGroups.filter((g) => !g.serverOrder.includes(server.uuid)),
    [serverGroups, server.uuid],
  );

  const { handleBulkPowerAction, bulkActionLoading } = useBulkPowerActions();

  const state = stats?.state;
  const powerBlocked = !!server.status || server.isSuspended || server.isTransferring || server.nodeMaintenanceEnabled;

  const permissionSet = useMemo(
    () => new Set([...server.permissions, ...(user?.role?.serverPermissions ?? [])]),
    [server.permissions, user?.role?.serverPermissions],
  );
  const canPower = useCallback(
    (action: string) => permissionSet.has('*') || permissionSet.has(action),
    [permissionSet],
  );

  const doPowerAction = useCallback(
    (action: z.infer<typeof serverPowerAction>) => handleBulkPowerAction([server.uuid], action),
    [handleBulkPowerAction, server.uuid],
  );

  const diskLimit = server.limits.disk !== 0 ? bytesToString(mbToBytes(server.limits.disk)) : t('common.unlimited', {});
  const memoryLimit =
    server.limits.memory !== 0 ? bytesToString(mbToBytes(server.limits.memory)) : t('common.unlimited', {});
  const cpuLimit = server.limits.cpu !== 0 ? `${server.limits.cpu}%` : t('common.unlimited', {});

  // plugged.host: the card's state line and meters (Theme v2's ThemeV2ServerEntry).
  const stateLabel = server.isSuspended
    ? t('common.server.state.suspended', {})
    : server.isTransferring
      ? t('common.server.state.transferring', {})
      : server.nodeMaintenanceEnabled
        ? t('common.server.state.nodeMaintenance', {})
        : server.status
          ? serverStatusInfo[server.status].label()
          : stats
            ? t(`common.enum.serverState.${stats.state}`, {})
            : statsUnavailable
              ? t('common.server.state.resourcesUnavailable', {})
              : t('common.enum.serverState.unknown', {});
  const stateFailed =
    server.isSuspended ||
    server.nodeMaintenanceEnabled ||
    (!!server.status && serverStatusInfo[server.status].failed) ||
    (!server.status && !stats && statsUnavailable);
  const stateBusy = !stateFailed && (server.isTransferring || !!server.status || (!stats && !statsUnavailable));

  const showUsage = !!stats && !server.isSuspended && !server.isTransferring && !server.status;
  const meter = (label: string, used: number, limit: number, format: (value: number) => string, unlimited: string) => {
    const percent = showUsage && limit > 0 ? Math.min(100, Math.max(0, (used / limit) * 100)) : 0;

    return {
      label,
      used: showUsage ? format(used) : null,
      limit: limit > 0 ? format(limit) : unlimited,
      percent,
      level: percent >= 95 ? 'danger' : percent >= 80 ? 'warning' : 'normal',
    };
  };
  const meters = [
    meter(t('common.stat.cpu', {}), stats?.cpuAbsolute ?? 0, server.limits.cpu, (v) => `${v.toFixed(2)}%`, cpuLimit),
    meter(
      t('common.stat.memoryUsage', {}),
      stats?.memoryBytes ?? 0,
      mbToBytes(server.limits.memory),
      bytesToString,
      memoryLimit,
    ),
    meter(
      t('common.stat.diskUsage', {}),
      stats?.diskBytes ?? 0,
      mbToBytes(server.limits.disk),
      bytesToString,
      diskLimit,
    ),
  ];

  const contextMenuItems: ContextMenuItem[] = useMemo(
    () => [
      {
        type: 'action' as const,
        icon: faPlay,
        label: t('common.enum.serverPowerAction.start', {}),
        color: 'gray',
        canAccess: canPower('control.start'),
        disabled: powerBlocked || bulkActionLoading !== null || state !== 'offline',
        onClick: () => doPowerAction('start'),
      },
      {
        type: 'action' as const,
        icon: faRotateRight,
        label: t('common.enum.serverPowerAction.restart', {}),
        canAccess: canPower('control.restart'),
        disabled: powerBlocked || bulkActionLoading !== null || !state,
        onClick: () => doPowerAction('restart'),
      },
      {
        type: 'action' as const,
        icon: faStop,
        label: t('common.enum.serverPowerAction.stop', {}),
        color: 'red',
        canAccess: canPower('control.stop'),
        disabled: powerBlocked || bulkActionLoading !== null || !state || state === 'offline',
        onClick: () => doPowerAction('stop'),
      },
      {
        type: 'action' as const,
        icon: faSkull,
        label: t('common.enum.serverPowerAction.kill', {}),
        color: 'red',
        hidden: state !== 'stopping',
        canAccess: canPower('control.stop'),
        disabled: powerBlocked || bulkActionLoading !== null,
        onClick: () => setOpenModal('kill'),
      },
    ],
    [t, doPowerAction, canPower, powerBlocked, bulkActionLoading, state],
  );

  return (
    <>
      <ServerAddGroupModal server={server} opened={openModal === 'add-group'} onClose={() => setOpenModal(null)} />

      <ConfirmationModal
        opened={openModal === 'kill'}
        onClose={() => setOpenModal(null)}
        title={t('pages.server.console.power.modal.forceStop.title', {})}
        confirm={t('common.button.continue', {})}
        onConfirmed={() => doPowerAction('kill')}
      >
        {t('pages.server.console.power.modal.forceStop.content', {}).md()}
      </ConfirmationModal>

      {/* plugged.host: Theme v2's server card ("phv2-server", server-entry.blade.php on the Pelican
          panel): a status bar down its left edge, the name over the state, the power menu at the end
          of that row, then CPU, memory and disk as used / limit over a thin bar, and the address.
          Its layout follows its own width (a container query in app.css). */}
      <ContextMenu enabled={showContextMenu} items={contextMenuItems}>
        {({ items, openMenu }) => (
          <div
            className='min-w-0'
            onClick={onClick}
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(e.clientX, e.clientY);
            }}
          >
            <NavLink
              to={to ?? `/server/${server.uuidShort}`}
              className='ph-server-card'
              data-selected={isSelected || undefined}
              onClick={(e) => {
                if (sKeyPressedRef?.current) {
                  e.preventDefault();
                }
              }}
            >
              <div className='phv2-server'>
                <span className={classNames('phv2-server-status-bar', statusToColor(stats?.state))} />

                <div className='phv2-server-head'>
                  {showSelection && (
                    <span
                      className='phv2-server-select'
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        onSelectionChange?.(!isSelected);
                      }}
                    >
                      <Checkbox
                        checked={isSelected}
                        readOnly
                        aria-label={
                          isSelected
                            ? t('pages.account.home.bulkActions.deselect', {})
                            : t('pages.account.home.bulkActions.select', {})
                        }
                        classNames={{ input: 'cursor-pointer!' }}
                      />
                    </span>
                  )}

                  <div className='phv2-server-title'>
                    <h2 className='phv2-server-name'>
                      <ScrollingText className='min-w-0 flex-1'>{server.name}</ScrollingText>
                      {showForeignServerBadge && !server.isOwner && (
                        <Tooltip label={t('pages.account.home.tooltip.foreign', {})}>
                          <FontAwesomeIcon
                            size='sm'
                            icon={faUsers}
                            className='shrink-0 text-(--mantine-color-yellow-text)'
                          />
                        </Tooltip>
                      )}
                      {!serverListShowOthers && serverGroups.every((g) => !g.serverOrder.includes(server.uuid)) && (
                        <Tooltip label={t('pages.account.home.tooltip.noGroup', {})}>
                          <FontAwesomeIcon
                            size='sm'
                            icon={faInfoCircle}
                            className='shrink-0 text-(--phv2-text-faint)'
                          />
                        </Tooltip>
                      )}
                    </h2>
                    <p className='phv2-server-state'>
                      {stateBusy && <Spinner size={12} />}
                      {stateFailed && (
                        <FontAwesomeIcon icon={faTriangleExclamation} className='text-(--mantine-color-yellow-text)' />
                      )}
                      <span className='truncate'>{stateLabel}</span>
                    </p>
                  </div>

                  <div className='phv2-server-actions'>
                    {showGroupAddButton && (
                      <Tooltip
                        label={
                          availableServerGroups.length === 0
                            ? t('pages.account.home.tooltip.noGroups', {})
                            : t('pages.account.home.tooltip.addToGroup', {})
                        }
                      >
                        <ActionIcon
                          size='input-sm'
                          variant='subtle'
                          color='gray'
                          aria-label={t('pages.account.home.tooltip.addToGroup', {})}
                          disabled={availableServerGroups.length === 0}
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            setOpenModal('add-group');
                          }}
                        >
                          <FontAwesomeIcon icon={faAdd} />
                        </ActionIcon>
                      </Tooltip>
                    )}
                    {onGroupRemove && (
                      <Tooltip label={t('pages.account.home.tooltip.removeFromGroup', {})}>
                        <ActionIcon
                          size='input-sm'
                          color='red'
                          variant='subtle'
                          aria-label={t('pages.account.home.tooltip.removeFromGroup', {})}
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            onGroupRemove();
                          }}
                        >
                          <FontAwesomeIcon icon={faMinus} />
                        </ActionIcon>
                      </Tooltip>
                    )}
                    {showContextMenu && items.some((item) => !item.hidden && item.canAccess !== false) && (
                      <Tooltip label={t('common.form.powerAction', {})}>
                        <ActionIcon
                          size='input-sm'
                          variant='subtle'
                          color='gray'
                          aria-label={t('common.form.powerAction', {})}
                          loading={bulkActionLoading !== null}
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            const rect = e.currentTarget.getBoundingClientRect();
                            openMenu(rect.left, rect.bottom);
                          }}
                        >
                          <FontAwesomeIcon icon={faEllipsisVertical} />
                        </ActionIcon>
                      </Tooltip>
                    )}
                  </div>
                </div>

                {server.description && <p className='phv2-server-description'>{server.description}</p>}

                <div className='phv2-server-stats'>
                  {meters.map((meter) => (
                    <div key={meter.label} className='phv2-server-stat'>
                      <div className='phv2-server-stat-info'>
                        <span className='phv2-server-stat-name'>{meter.label}</span>
                        <span className='phv2-server-stat-value'>
                          {meter.used === null ? (
                            <span className='phv2-server-stat-limit'>—</span>
                          ) : (
                            <>
                              <span className={classNames('phv2-server-stat-used', `phv2-server-stat-${meter.level}`)}>
                                {meter.used}
                              </span>{' '}
                              <span className='phv2-server-stat-limit'>/ {meter.limit}</span>
                            </>
                          )}
                        </span>
                      </div>
                      <div
                        className='phv2-server-track'
                        role='progressbar'
                        aria-label={`${meter.label}: ${meter.used ?? '—'} / ${meter.limit}`}
                        aria-valuenow={Math.round(meter.percent)}
                        aria-valuemin={0}
                        aria-valuemax={100}
                      >
                        <span
                          className={classNames('phv2-server-bar', `phv2-server-bar-${meter.level}`)}
                          style={{ width: `${meter.percent}%` }}
                        />
                      </div>
                    </div>
                  ))}

                  <div className='phv2-server-address'>
                    <span className='phv2-server-stat-name'>{t('common.stat.network', {})}</span>
                    {server.allocation ? (
                      <CopyOnClick content={formatAllocation(server.allocation)} className='min-w-0'>
                        <span className='phv2-server-address-value'>
                          <RedactedText value={formatAllocation(server.allocation)} />
                        </span>
                      </CopyOnClick>
                    ) : (
                      <span className='phv2-server-address-value'>{t('common.server.noAllocation', {})}</span>
                    )}
                  </div>
                </div>
              </div>
            </NavLink>
          </div>
        )}
      </ContextMenu>
    </>
  );
}
