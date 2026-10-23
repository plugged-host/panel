// plugged.host: Admin › Overview › Capacity: what each location can still take, and the moves
// under way (docs/features/transfers.md).
import getCapacity from '@/api/admin/system/getCapacity.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const statusColors: Record<string, string> = {
  queued: 'gray',
  running: 'blue',
  cancelling: 'yellow',
  completed: 'green',
  failed: 'red',
  cancelled: 'gray',
};

const size = (mb: number) => bytesToString(mbToBytes(mb));

export default function AdminOverviewCapacity() {
  const { t } = useTranslations();
  const page = 'pages.admin.home.tabs.capacity.page';

  const capacity = useResource({
    queryKey: queryKeys.admin.health.capacity(),
    queryFn: getCapacity,
  });

  return (
    <ResourceView resource={capacity}>
      {({ locations, transfers }) => (
        <div className='flex flex-col gap-4'>
          <Text size='sm' c='dimmed'>
            {t(`${page}.description`, {})}
          </Text>

          <TitleCard title={t(`${page}.card.locations`, {})}>
            <Table
              columns={[
                t('common.table.columns.location', {}),
                t(`${page}.table.nodes`, {}),
                t(`${page}.table.memoryFree`, {}),
                t(`${page}.table.diskFree`, {}),
                t(`${page}.table.pressure`, {}),
              ]}
            >
              {locations.map((location) => (
                <TableRow key={location.uuid}>
                  <TableData>
                    <TableLink to={`/admin/locations/${location.uuid}`}>{location.name}</TableLink>
                  </TableData>
                  <TableData>{t(`${page}.openNodes`, { open: location.openNodes, total: location.nodes })}</TableData>
                  <TableData>
                    {location.memoryTotal > 0
                      ? `${size(location.memoryFree)} / ${size(location.memoryTotal)}`
                      : t('common.unlimited', {})}
                  </TableData>
                  <TableData>
                    {location.diskTotal > 0
                      ? `${size(location.diskFree)} / ${size(location.diskTotal)}`
                      : t('common.unlimited', {})}
                  </TableData>
                  <TableData>
                    {location.pressure !== null ? `${location.pressure.toFixed(0)}%` : t('common.na', {})}
                  </TableData>
                </TableRow>
              ))}
            </Table>
          </TitleCard>

          <TitleCard title={t(`${page}.card.transfers`, {})}>
            {transfers.length === 0 ? (
              <Text size='sm' c='dimmed'>
                {t(`${page}.noTransfers`, {})}
              </Text>
            ) : (
              <Table
                columns={[
                  t('common.table.columns.server', {}),
                  t(`${page}.table.route`, {}),
                  t('common.table.columns.status', {}),
                  t(`${page}.table.origin`, {}),
                  t('common.table.columns.created', {}),
                ]}
              >
                {transfers.map((transfer) => (
                  <TableRow key={transfer.uuid}>
                    <TableData>
                      <TableLink to={`/admin/servers/${transfer.serverUuid}`}>
                        <Code>{transfer.serverName}</Code>
                      </TableLink>
                    </TableData>
                    <TableData>
                      {transfer.sourceNodeName ?? t('common.na', {})} →{' '}
                      {transfer.destinationNodeName ?? t('common.na', {})}
                    </TableData>
                    <TableData>
                      <Badge color={statusColors[transfer.status]} variant='light'>
                        {t(`${page}.status.${transfer.status}`, {})}
                      </Badge>
                      {transfer.attempts > 1 && (
                        <Text size='xs' c='dimmed'>
                          {t(`${page}.attempts`, { attempts: transfer.attempts })}
                        </Text>
                      )}
                      {transfer.error && (
                        <Text size='xs' c='dimmed'>
                          {transfer.error}
                        </Text>
                      )}
                    </TableData>
                    <TableData>{t(`${page}.origin.${transfer.origin}`, {})}</TableData>
                    <TableData>
                      <FormattedTimestamp timestamp={transfer.created} />
                    </TableData>
                  </TableRow>
                ))}
              </Table>
            )}
          </TitleCard>
        </div>
      )}
    </ResourceView>
  );
}
