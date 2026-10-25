// plugged.host: a node's telemetry (shared/src/node_telemetry.rs): its state now and its history.
import { AreaChart } from '@mantine/charts';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { z } from 'zod';
import getNodeTelemetry from '@/api/admin/nodes/getNodeTelemetry.ts';
import InfoRow from '@/elements/admin/InfoRow.tsx';
import ChartBlock from '@/elements/charts/ChartBlock.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import SegmentedControl from '@/elements/layout/SegmentedControl.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminNodeSchema, adminNodeTelemetrySchema } from '@/lib/schemas/admin/nodes.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

import '@mantine/charts/styles.css';
import { intlLocale } from '@/lib/languages.ts';

const RANGES = [
  { hours: 1, key: 'hour' },
  { hours: 24, key: 'day' },
  { hours: 168, key: 'week' },
] as const;
const REFRESH = 60_000;

type Latest = z.infer<typeof adminNodeTelemetrySchema>;

function percent(used: number | null, total: number | null): number | null {
  return used === null || !total ? null : (used / total) * 100;
}

function HistoryChart({
  title,
  value,
  data,
  name,
  label,
  unit,
  max,
}: {
  title: string;
  value: string | null;
  data: Record<string, number | string | null>[];
  name: string;
  label: string;
  unit: string;
  max?: number;
}) {
  return (
    <ChartBlock title={title} value={value}>
      <AreaChart
        h={220}
        data={data}
        dataKey='time'
        series={[{ name, label, color: 'var(--chart-series-1)' }]}
        curveType='monotone'
        fillOpacity={0.25}
        strokeWidth={2}
        withDots={false}
        gridAxis='y'
        connectNulls={false}
        unit={unit}
        valueFormatter={(v) => (Number.isInteger(v) ? String(v) : v.toFixed(1))}
        xAxisProps={{ minTickGap: 48 }}
        yAxisProps={{ domain: [0, max ?? 'auto'], width: 48 }}
      />
    </ChartBlock>
  );
}

export default function NodeTelemetryHistory({ node }: { node: z.infer<typeof adminNodeSchema> }) {
  const { t, language } = useTranslations();
  const [hours, setHours] = useState(24);

  const { data, isLoading } = useQuery({
    queryKey: queryKeys.admin.nodes.telemetry(node.uuid, hours),
    queryFn: () => getNodeTelemetry(node.uuid, hours),
    refetchInterval: REFRESH,
    placeholderData: keepPreviousData,
  });

  const rows = useMemo(() => {
    const format = new Intl.DateTimeFormat(
      intlLocale(language),
      hours > 24 ? { weekday: 'short', hour: '2-digit', minute: '2-digit' } : { hour: '2-digit', minute: '2-digit' },
    );

    return (data?.history ?? []).map((point) => ({
      time: format.format(point.sampled),
      cpu: point.cpuPercent,
      memory: percent(point.memoryUsed, point.memoryTotal),
      disk: percent(point.diskUsed, point.diskTotal),
      response: point.responseTimeMs,
      servers: point.serversRunning,
    }));
  }, [data, hours, language]);

  const status = (latest: Latest) =>
    latest.reachable ? (
      <Badge color='green' variant='light'>
        {t('pages.admin.nodes.tabs.statistics.page.telemetry.status.answering', {})}
      </Badge>
    ) : latest.refused ? (
      <Badge color='yellow' variant='light'>
        {t('pages.admin.nodes.tabs.statistics.page.telemetry.status.refused', {})}
      </Badge>
    ) : (
      <Badge color='red' variant='light'>
        {t('pages.admin.nodes.tabs.statistics.page.telemetry.status.unreachable', {})}
      </Badge>
    );

  const latest = data?.latest ?? null;
  const shown = (value: number | null) => (value === null ? null : `${value.toFixed(1)}%`);
  const na = (
    <Text span size='sm' c='dimmed'>
      {t('common.na', {})}
    </Text>
  );

  return (
    <div className='mt-4'>
      <TitleCard
        title={t('pages.admin.nodes.tabs.statistics.page.telemetry.title', {})}
        rightSection={
          <div className='ms-auto'>
            <SegmentedControl
              size='xs'
              value={String(hours)}
              onChange={(value) => setHours(Number(value))}
              data={RANGES.map((range) => ({
                value: String(range.hours),
                label: t(`pages.admin.nodes.tabs.statistics.page.telemetry.range.${range.key}`, {}),
              }))}
            />
          </div>
        }
      >
        {isLoading ? (
          <Spinner.Centered />
        ) : !latest ? (
          <Text size='sm' c='dimmed'>
            {t('pages.admin.nodes.tabs.statistics.page.telemetry.empty', {})}
          </Text>
        ) : (
          <Stack gap='md'>
            <div className='grid grid-cols-1 lg:grid-cols-2 gap-x-8'>
              <Stack gap={0}>
                <InfoRow label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.status', {})}>
                  {status(latest)}
                </InfoRow>
                <InfoRow label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.responseTime', {})}>
                  {latest.responseTimeMs === null
                    ? na
                    : t('pages.admin.nodes.tabs.statistics.page.telemetry.value.milliseconds', {
                        ms: latest.responseTimeMs,
                      })}
                </InfoRow>
                <InfoRow label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.pressure', {})}>
                  {latest.pressure === null ? na : `${latest.pressure.toFixed(0)}%`}
                </InfoRow>
                {!latest.reachable && (
                  <InfoRow label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.failures', {})}>
                    {latest.consecutiveFailures}
                  </InfoRow>
                )}
              </Stack>
              <Stack gap={0}>
                <InfoRow label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.load', {})}>
                  {latest.load1 === null
                    ? na
                    : latest.cpuThreads
                      ? `${latest.load1.toFixed(2)} / ${latest.cpuThreads}`
                      : latest.load1.toFixed(2)}
                </InfoRow>
                <InfoRow label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.serversRunning', {})}>
                  {latest.serversRunning ?? na}
                </InfoRow>
                <InfoRow label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.lastReachable', {})}>
                  {latest.lastReachable ? formatDateTime(latest.lastReachable) : na}
                </InfoRow>
                <InfoRow label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.sampled', {})}>
                  {formatDateTime(latest.sampled)}
                </InfoRow>
              </Stack>
            </div>
            {latest.error && (
              <Text size='sm' c='dimmed' ff='monospace' className='break-all'>
                {latest.error}
              </Text>
            )}
            <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
              <HistoryChart
                title={t('common.stat.cpuLoad', {})}
                value={shown(latest.cpuPercent)}
                data={rows}
                name='cpu'
                label={t('common.stat.cpu', {})}
                unit='%'
                max={100}
              />
              <HistoryChart
                title={t('common.stat.memoryUsage', {})}
                value={shown(percent(latest.memoryUsed, latest.memoryTotal))}
                data={rows}
                name='memory'
                label={t('pages.admin.nodes.tabs.statistics.page.label.memory', {})}
                unit='%'
                max={100}
              />
              <HistoryChart
                title={t('pages.admin.nodes.tabs.statistics.page.telemetry.chart.disk', {})}
                value={shown(percent(latest.diskUsed, latest.diskTotal))}
                data={rows}
                name='disk'
                label={t('pages.admin.nodes.tabs.statistics.page.label.disk', {})}
                unit='%'
                max={100}
              />
              <HistoryChart
                title={t('pages.admin.nodes.tabs.statistics.page.telemetry.chart.responseTime', {})}
                value={
                  latest.responseTimeMs === null
                    ? null
                    : t('pages.admin.nodes.tabs.statistics.page.telemetry.value.milliseconds', {
                        ms: latest.responseTimeMs,
                      })
                }
                data={rows}
                name='response'
                label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.responseTime', {})}
                unit=' ms'
              />
              <HistoryChart
                title={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.serversRunning', {})}
                value={latest.serversRunning === null ? null : String(latest.serversRunning)}
                data={rows}
                name='servers'
                label={t('pages.admin.nodes.tabs.statistics.page.telemetry.label.serversRunning', {})}
                unit=''
              />
            </div>
          </Stack>
        )}
      </TitleCard>
    </div>
  );
}
