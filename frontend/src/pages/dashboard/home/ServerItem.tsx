import {
  faAdd,
  faMinus,
  faPlay,
  faPowerOff,
  faRotateRight,
  faSkull,
  faStop,
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
import { isInLimbo } from '@/lib/domain/limbo.ts'; // plugged.host
import { formatAllocation } from '@/lib/domain/server.ts';
import { serverCondition } from '@/lib/domain/serverCondition.ts'; // plugged.host
import { formatSizePair, formatUptimeShort, usageBarPercent, usageLevel } from '@/lib/format/serverUsage.ts'; // plugged.host
import { serverPowerAction, serverSchema } from '@/lib/schemas/server/server.ts';
import { useBulkPowerActions } from '@/plugins/server/useBulkPowerActions.ts';
import { useServerStats, useServerStatsUnavailable } from '@/plugins/server/useServerStats.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useUserStore } from '@/stores/user.ts';
import ServerAddGroupModal from './modals/ServerAddGroupModal.tsx';

// plugged.host: what the card shows of a limit it does not have, and how long a description gets
// (Pelican's Str::limit(description, 80, preserveWords: true))
const UNLIMITED = '∞';
const DESCRIPTION_LIMIT = 80;

function limitWords(text: string, limit: number): string {
  if (Array.from(text).length <= limit) return text;

  const characters = Array.from(text.replace(/[\n\r]+/g, ' ').trim());
  if (characters.length <= limit) return characters.join('');

  const cut = characters.slice(0, limit).join('').trimEnd();
  if (characters[limit] === ' ') return `${cut}...`;

  // drop the word the limit cut through
  const lastSpace = cut.search(/\s\S*$/);

  return `${lastSpace > 0 ? cut.slice(0, lastSpace) : cut}...`;
}

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

  // plugged.host: the card's state line and meters (Theme v2's ThemeV2ServerEntry): "Running · 2h 14m",
  // "Loading..." until the stats arrive, and "12 / 100%", "1.2 / 2 GiB" with Pelican's 70 % and 90 % marks.
  const condition = serverCondition(server, state, statsUnavailable);
  const loading = condition.label === null;
  const uptime = stats?.uptime ?? 0;
  const stateLabel =
    condition.label === null
      ? t('pages.account.home.card.loading', {})
      : condition.live && uptime > 0
        ? t('pages.account.home.card.stateUptime', { state: condition.label, uptime: formatUptimeShort(uptime) })
        : condition.label;

  const limbo = isInLimbo(server.limboState); // plugged.host: docs/features/limbo.md
  const icon = server.icon ?? server.eggIcon; // plugged.host: its own icon, else its game's
  const showUsage = !!stats && !server.isSuspended && !server.isTransferring && !server.status && !limbo;
  const cpuLimit = server.limits.cpu + server.limits.burstCpu; // plugged.host: burst cores above the paid CPU
  const cpuUsed = Math.round(stats?.cpuAbsolute ?? 0);
  const memoryUsed = Math.round((stats?.memoryBytes ?? 0) / 1048576);
  const diskUsed = Math.round((stats?.diskBytes ?? 0) / 1048576);
  const [memoryUsedText, memoryLimitText] = formatSizePair(memoryUsed, server.limits.memory, UNLIMITED);
  const [diskUsedText, diskLimitText] = formatSizePair(diskUsed, server.limits.disk, UNLIMITED);

  const meter = (label: string, used: number, limit: number, usedText: string, limitText: string) => ({
    label,
    used: showUsage ? usedText : null,
    limit: limitText,
    percent: showUsage ? usageBarPercent(used, limit) : 0,
    level: showUsage ? usageLevel(used, limit) : 'normal',
  });
  const meters = [
    meter(
      t('pages.account.home.card.cpu', {}),
      cpuUsed,
      server.limits.cpu > 0 ? cpuLimit : 0,
      cpuUsed.toString(),
      server.limits.cpu === 0
        ? UNLIMITED
        : server.limits.burstCpu > 0
          ? t('common.stat.cpuWithBurst', { base: `${server.limits.cpu}%`, burst: `${server.limits.burstCpu}%` })
          : `${server.limits.cpu}%`,
    ),
    meter(t('pages.account.home.card.memory', {}), memoryUsed, server.limits.memory, memoryUsedText, memoryLimitText),
    meter(t('pages.account.home.card.disk', {}), diskUsed, server.limits.disk, diskUsedText, diskLimitText),
  ];

  // plugged.host: Pelican's power menu (ListServers::getPowerActionGroup): only the actions the state
  // allows are listed (Start when offline, Restart, Stop while running, Kill while starting or
  // stopping), and none while the server is busy elsewhere.
  const powerHidden = powerBlocked || !state;
  const contextMenuItems: ContextMenuItem[] = useMemo(
    () => [
      {
        type: 'action' as const,
        icon: faPlay,
        label: t('common.enum.serverPowerAction.start', {}),
        color: 'blue',
        hidden: powerHidden || state !== 'offline',
        canAccess: canPower('control.start'),
        disabled: bulkActionLoading !== null,
        onClick: () => doPowerAction('start'),
      },
      {
        type: 'action' as const,
        icon: faRotateRight,
        label: t('common.enum.serverPowerAction.restart', {}),
        hidden: powerHidden,
        canAccess: canPower('control.restart'),
        disabled: bulkActionLoading !== null,
        onClick: () => doPowerAction('restart'),
      },
      {
        type: 'action' as const,
        icon: faStop,
        label: t('common.enum.serverPowerAction.stop', {}),
        color: 'red',
        hidden: powerHidden || state !== 'running',
        canAccess: canPower('control.stop'),
        disabled: bulkActionLoading !== null,
        onClick: () => doPowerAction('stop'),
      },
      {
        type: 'action' as const,
        icon: faSkull,
        label: t('common.enum.serverPowerAction.kill', {}),
        color: 'red',
        hidden: powerHidden || (state !== 'starting' && state !== 'stopping'),
        canAccess: canPower('control.stop'),
        disabled: bulkActionLoading !== null,
        onClick: () => setOpenModal('kill'),
      },
    ],
    [t, doPowerAction, canPower, powerHidden, bulkActionLoading, state],
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

      {/* plugged.host: Theme v2's server card ("theme-server", server-entry.blade.php on the Pelican
          panel): a status bar down its left edge in the condition's colour, the icon, the name over
          the state, the power menu at the end of that row, then CPU, memory and disk as used / limit
          over a thin bar, and the address. Its layout follows its own width (a container query in
          app.css). */}
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
              className='theme-server-card'
              data-selected={isSelected || undefined}
              onClick={(e) => {
                if (sKeyPressedRef?.current) {
                  e.preventDefault();
                }
              }}
            >
              <div className='theme-server'>
                <span className='theme-server-status-bar' data-color={condition.color} />

                <div className='theme-server-head'>
                  {showSelection && (
                    <span
                      className='theme-server-select'
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

                  {icon && <img className='theme-server-icon' src={icon} alt='' loading='lazy' />}

                  <div className='theme-server-title'>
                    <h2 className='theme-server-name'>
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
                    </h2>
                    <p className='theme-server-state'>
                      {loading && <Spinner size={14} />}
                      <span className='truncate'>{stateLabel}</span>
                    </p>
                  </div>

                  <div className='theme-server-actions'>
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
                      <Tooltip label={t('pages.account.home.card.powerActions', {})}>
                        <ActionIcon
                          size='input-sm'
                          variant='subtle'
                          color='blue'
                          aria-label={t('pages.account.home.card.powerActions', {})}
                          loading={bulkActionLoading !== null}
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            const rect = e.currentTarget.getBoundingClientRect();
                            openMenu(rect.left, rect.bottom);
                          }}
                        >
                          <FontAwesomeIcon icon={faPowerOff} size='xl' />
                        </ActionIcon>
                      </Tooltip>
                    )}
                  </div>
                </div>

                {server.description && (
                  <p className='theme-server-description'>{limitWords(server.description, DESCRIPTION_LIMIT)}</p>
                )}

                <div className='theme-server-stats'>
                  {meters.map((meter) => (
                    <div key={meter.label} className='theme-server-stat'>
                      <div className='theme-server-stat-info'>
                        <span className='theme-server-stat-name'>{meter.label}</span>
                        <span className='theme-server-stat-value'>
                          {meter.used === null ? (
                            <span className='theme-server-stat-limit'>—</span>
                          ) : (
                            <>
                              <span
                                className={classNames('theme-server-stat-used', `theme-server-stat-${meter.level}`)}
                              >
                                {meter.used}
                              </span>{' '}
                              <span className='theme-server-stat-limit'>/ {meter.limit}</span>
                            </>
                          )}
                        </span>
                      </div>
                      <div
                        className='theme-server-track'
                        role='progressbar'
                        aria-label={`${meter.label}: ${meter.used ?? '—'} / ${meter.limit}`}
                        aria-valuenow={meter.percent}
                        aria-valuemin={0}
                        aria-valuemax={100}
                      >
                        <span
                          className={classNames('theme-server-bar', `theme-server-bar-${meter.level}`)}
                          style={{ width: `${meter.percent}%` }}
                        />
                      </div>
                    </div>
                  ))}

                  <div className='theme-server-address'>
                    <span className='theme-server-stat-name'>{t('common.stat.network', {})}</span>
                    {server.allocation ? (
                      <CopyOnClick content={formatAllocation(server.allocation)} className='min-w-0'>
                        <span className='theme-server-address-value'>
                          <RedactedText value={formatAllocation(server.allocation)} />
                        </span>
                      </CopyOnClick>
                    ) : (
                      <span className='theme-server-address-value'>{t('common.none', {})}</span>
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
