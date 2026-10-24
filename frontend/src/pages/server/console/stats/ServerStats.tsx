import { useEffect, useMemo, useRef } from 'react';
import ChartBlock from '@/elements/charts/ChartBlock.tsx';
import ChartLegend from '@/elements/charts/ChartLegend.tsx';
import ChartSyncGroup from '@/elements/charts/ChartSyncGroup.tsx';
import StreamChart from '@/elements/charts/StreamChart.tsx';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import { formatBytes, formatPercent, useStreamChart } from '@/lib/chart.ts';
import { bitsPerSecondToString, mbToBytes } from '@/lib/format/size.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export default function ServerStats() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const stats = useServerStore((state) => state.stats);

  const networkPrevious = useRef<{
    tx: number;
    rx: number;
    uptime: number;
    timestamp: number;
  } | null>(null);
  const wasOffline = useRef(false);

  const cpu = useStreamChart({
    series: useMemo(() => [t('common.stat.cpuLoad', {})], [t]),
    format: formatPercent,
    min: 10,
    limit: server.limits.cpu > 0 ? server.limits.cpu + server.limits.burstCpu : null, // plugged.host: with burst
  });
  const memory = useStreamChart({
    series: useMemo(() => [t('common.stat.memoryLoad', {})], [t]),
    format: formatBytes,
    scale: 'binary',
    min: mbToBytes(64),
    limit: server.limits.memory > 0 ? mbToBytes(server.limits.memory) : null,
  });
  const network = useStreamChart({
    series: useMemo(() => [t('common.stat.outbound', {}), t('common.stat.inbound', {})], [t]),
    format: bitsPerSecondToString,
    min: 1000,
  });

  const offline = !stats?.state || (stats.state === 'offline' && server.status !== 'installing');

  useEffect(() => {
    if (offline) {
      networkPrevious.current = null;
      if (!wasOffline.current) {
        wasOffline.current = true;
        cpu.push(0);
        memory.push(0);
        network.push([0, 0]);
      }
      return;
    }

    wasOffline.current = false;
    cpu.push(stats.cpuAbsolute);
    memory.push(stats.memoryBytes);
    const now = performance.now();
    const previous = networkPrevious.current;
    const elapsedSeconds = previous ? (now - previous.timestamp) / 1000 : 0;
    const canCalculateRate =
      previous &&
      elapsedSeconds > 0 &&
      stats.uptime >= previous.uptime &&
      stats.network.txBytes >= previous.tx &&
      stats.network.rxBytes >= previous.rx;

    network.push(
      canCalculateRate
        ? [
            ((stats.network.txBytes - previous.tx) * 8) / elapsedSeconds,
            ((stats.network.rxBytes - previous.rx) * 8) / elapsedSeconds,
          ]
        : [0, 0],
    );

    networkPrevious.current = {
      tx: stats.network.txBytes,
      rx: stats.network.rxBytes,
      uptime: stats.uptime,
      timestamp: now,
    };
  }, [stats, offline, cpu.push, memory.push, network.push]);

  // plugged.host: the Pelican console's CPU, Memory and Network charts: titles only (no icons, no
  // value), "gap-6", and not shown at all while the server is offline (its widgets' canView()).
  if (offline) {
    return null;
  }

  return (
    <ChartSyncGroup>
      <div className='grid grid-cols-1 md:grid-cols-3 gap-6'>
        <ChartBlock title={t('pages.server.console.stats.cpu', {})}>
          <StreamChart {...cpu.props} />
        </ChartBlock>
        <ChartBlock title={t('pages.server.console.stats.memory', {})}>
          <StreamChart {...memory.props} />
        </ChartBlock>
        <ChartBlock title={t('common.stat.network', {})} legend={<ChartLegend {...network.legend} />}>
          <StreamChart {...network.props} />
        </ChartBlock>
        <ExtensionSlot
          components={window.extensionContext.extensionRegistry.pages.server.console.statBlocks}
          name='console-stat-block'
        />
      </div>
    </ChartSyncGroup>
  );
}
