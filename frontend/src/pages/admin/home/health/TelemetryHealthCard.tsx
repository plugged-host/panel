// plugged.host: Admin › Overview › Health: nodes whose telemetry sample is missing or old, which
// means the sampler stopped or cannot finish (docs/features/health.md).
import { faHeartPulse } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import getTelemetryHealth from '@/api/admin/system/health/getTelemetryHealth.ts';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function TelemetryHealthCard() {
  const { t } = useTranslations();

  const telemetry = useResource({
    queryKey: queryKeys.admin.health.telemetry(),
    queryFn: getTelemetryHealth,
  });

  return (
    <TitleCard
      title={t('pages.admin.home.tabs.health.page.card.telemetry', {})}
      icon={<FontAwesomeIcon icon={faHeartPulse} />}
    >
      <ResourceView resource={telemetry}>
        {(health) =>
          health.stale.length === 0 ? (
            <Text size='sm' c='dimmed'>
              {t('pages.admin.home.tabs.health.page.telemetryFresh', { total: health.total })}
            </Text>
          ) : (
            <>
              <Text size='sm' c='yellow' mb='sm'>
                {t('pages.admin.home.tabs.health.page.telemetryStale', {
                  stale: health.stale.length,
                  total: health.total,
                  seconds: health.staleAfterSeconds,
                })}
              </Text>
              <Table
                columns={[
                  t('common.table.columns.node', {}),
                  t('pages.admin.home.tabs.health.page.table.lastSample', {}),
                ]}
              >
                {health.stale.map((node) => (
                  <TableRow key={node.uuid}>
                    <TableData>
                      <TableLink to={`/admin/nodes/${node.uuid}`}>
                        <Code>{node.name}</Code>
                      </TableLink>
                    </TableData>
                    <TableData>
                      {node.sampled ? <FormattedTimestamp timestamp={node.sampled} /> : t('common.na', {})}
                    </TableData>
                  </TableRow>
                ))}
              </Table>
            </>
          )
        }
      </ResourceView>
    </TitleCard>
  );
}
