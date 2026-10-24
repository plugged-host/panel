// plugged.host: the console's stat blocks as the Pelican panel has them (its ServerOverview widget): Name, Status, Address, CPU, Memory and Disk on a line each, over the terminal; replaces upstream's ServerDetails there
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import RedactedText from '@/elements/typography/RedactedText.tsx';
import { formatAllocation } from '@/lib/domain/server.ts';
import { serverCondition } from '@/lib/domain/serverCondition.ts';
import { formatNumber, formatUptimeShort } from '@/lib/format/serverUsage.ts';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useServerStatsPending } from './useServerStatsPending.ts';

// what a value shows while it is not known yet, and a limit the server does not have
const UNKNOWN = '—';
const UNLIMITED = '∞';

function OverviewBlock({
  label,
  value,
  copy,
  redact = false,
}: {
  label: string;
  value: string;
  copy?: string;
  redact?: boolean;
}) {
  const row = (
    <>
      <span className='theme-console-stat-label'>{label}</span>{' '}
      <span className='theme-console-stat-value'>{redact ? <RedactedText value={value} /> : value}</span>
    </>
  );

  return (
    <div className='theme-console-stat'>
      {copy ? (
        <CopyOnClick content={copy} className='theme-console-stat-row'>
          {row}
        </CopyOnClick>
      ) : (
        <span className='theme-console-stat-row'>{row}</span>
      )}
    </div>
  );
}

export default function ServerOverview() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const stats = useServerStore((state) => state.stats);
  const state = useServerStore((state) => state.state);
  const statsPending = useServerStatsPending();

  // an installing server's container runs, so its usage counts (as upstream's ServerDetails has it)
  const offline = !statsPending && state === 'offline' && server.status !== 'installing';
  const p = 'pages.server.console.details';

  const condition = serverCondition(server, statsPending ? null : state);
  const uptime = stats?.uptime ?? 0;
  const status =
    condition.label === null
      ? UNKNOWN
      : condition.live && !offline && uptime > 0
        ? t(`${p}.statusUptime`, { state: condition.label, uptime: formatUptimeShort(uptime) })
        : condition.label;

  const address = server.allocation ? formatAllocation(server.allocation, server.egg.separatePort) : null;

  const percent = (value: number) => `${formatNumber(value)} %`;
  const cpuLimit =
    server.limits.cpu === 0
      ? UNLIMITED
      : server.limits.burstCpu > 0 // burst cores above the paid CPU
        ? t('common.stat.cpuWithBurst', { base: percent(server.limits.cpu), burst: percent(server.limits.burstCpu) })
        : percent(server.limits.cpu);
  const memoryLimit = server.limits.memory > 0 ? bytesToString(mbToBytes(server.limits.memory)) : UNLIMITED;
  const diskLimit = server.limits.disk > 0 ? bytesToString(mbToBytes(server.limits.disk)) : UNLIMITED;

  // CPU and memory say Offline when the server is, as Pelican's console does; the disk keeps its size
  const usage = (value: (s: NonNullable<typeof stats>) => string, limit: string) =>
    offline
      ? t('common.enum.serverState.offline', {})
      : `${stats && !statsPending ? value(stats) : UNKNOWN} / ${limit}`;

  return (
    <div className='theme-console-overview'>
      <div className='theme-console-stats'>
        <OverviewBlock label={t(`${p}.name`, {})} value={server.name} copy={server.name} />
        <OverviewBlock label={t(`${p}.status`, {})} value={status} />
        <OverviewBlock
          label={t(`${p}.address`, {})}
          value={address ?? t('common.none', {})}
          copy={address ?? undefined}
          redact={!!address}
        />
        {server.egg.separatePort && server.allocation && (
          <OverviewBlock
            label={t(`${p}.port`, {})}
            value={server.allocation.port.toString()}
            copy={server.allocation.port.toString()}
          />
        )}
        <OverviewBlock label={t(`${p}.cpu`, {})} value={usage((s) => percent(s.cpuAbsolute), cpuLimit)} />
        <OverviewBlock label={t(`${p}.memory`, {})} value={usage((s) => bytesToString(s.memoryBytes), memoryLimit)} />
        <OverviewBlock
          label={t(`${p}.disk`, {})}
          value={`${stats && stats.diskBytes > 0 ? bytesToString(stats.diskBytes) : UNKNOWN} / ${diskLimit}`}
        />
        <ExtensionSlot
          components={window.extensionContext.extensionRegistry.pages.server.console.statCards}
          name='console-stat-card'
        />
      </div>
    </div>
  );
}
