// plugged.host: a node's Failover tab: restore its servers on other nodes of its location from
// their newest backup, a drill on one server, and the newest failovers (docs/features/failover.md).
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { z } from 'zod';
import cancelNodeFailover from '@/api/admin/nodes/failover/cancelNodeFailover.ts';
import getNodeFailovers from '@/api/admin/nodes/failover/getNodeFailovers.ts';
import startNodeFailover from '@/api/admin/nodes/failover/startNodeFailover.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminNodeFailoverSchema, adminServerFailoverSchema } from '@/lib/schemas/admin/failover.ts';
import { adminNodeSchema } from '@/lib/schemas/admin/nodes.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import FailoverDrillModal from './modals/FailoverDrillModal.tsx';

const failoverColors: Record<z.infer<typeof adminNodeFailoverSchema>['status'], string> = {
  running: 'blue',
  done: 'green',
  cancelled: 'gray',
};

const serverColors: Record<z.infer<typeof adminServerFailoverSchema>['status'], string> = {
  queued: 'gray',
  rehoming: 'blue',
  restoring: 'blue',
  done: 'green',
  failed: 'red',
  cancelled: 'gray',
};

export default function AdminNodeFailover({ node }: { node: z.infer<typeof adminNodeSchema> }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const page = 'pages.admin.nodes.tabs.failover.page';

  const failovers = useQuery({
    queryKey: queryKeys.admin.nodes.failovers(node.uuid),
    queryFn: () => getNodeFailovers(node.uuid),
    refetchInterval: (query) =>
      query.state.data?.failovers.some((failover) => failover.status === 'running') ? 15_000 : false,
  });

  const [openModal, setOpenModal] = useState<'start' | 'drill' | 'cancel' | null>(null);

  const running = failovers.data?.failovers.some((failover) => failover.status === 'running') ?? false;
  const off = failovers.data?.mode === 'off';

  const doStart = async (drillServerUuid: string | null) => {
    await startNodeFailover(node.uuid, { drillServerUuid });
    addToast(t(`${page}.toast.started`, {}), 'success');
    setOpenModal(null);
    failovers.refetch();
  };

  const doCancel = async () => {
    await cancelNodeFailover(node.uuid);
    addToast(t(`${page}.toast.cancelled`, {}), 'success');
    setOpenModal(null);
    failovers.refetch();
  };

  return (
    <AdminSubContentContainer
      title={t(`${page}.title`, {})}
      titleOrder={2}
      contentRight={
        <AdminCan action='nodes.failover'>
          <Group gap='xs'>
            {running ? (
              <Button variant='default' onClick={() => setOpenModal('cancel')}>
                {t(`${page}.button.cancel`, {})}
              </Button>
            ) : (
              <>
                <Button variant='default' disabled={off} onClick={() => setOpenModal('drill')}>
                  {t(`${page}.button.drill`, {})}
                </Button>
                <Button variant='default' disabled={off} onClick={() => setOpenModal('start')}>
                  {t(`${page}.button.start`, {})}
                </Button>
              </>
            )}
          </Group>
        </AdminCan>
      }
    >
      <ConfirmationModal
        opened={openModal === 'start'}
        onClose={() => setOpenModal(null)}
        title={t(`${page}.modal.start.title`, {})}
        confirm={t(`${page}.button.start`, {})}
        onConfirmed={() => doStart(null)}
      >
        {t(`${page}.modal.start.content`, { node: node.name }).md()}
      </ConfirmationModal>

      <ConfirmationModal
        opened={openModal === 'cancel'}
        onClose={() => setOpenModal(null)}
        title={t(`${page}.modal.cancel.title`, {})}
        confirm={t(`${page}.button.cancel`, {})}
        onConfirmed={doCancel}
      >
        {t(`${page}.modal.cancel.content`, {}).md()}
      </ConfirmationModal>

      <FailoverDrillModal
        node={node}
        opened={openModal === 'drill'}
        onClose={() => setOpenModal(null)}
        onConfirmed={(serverUuid) => doStart(serverUuid)}
      />

      {failovers.isLoading ? (
        <Spinner.Centered />
      ) : failovers.error ? (
        <Alert color='red'>{httpErrorToHuman(failovers.error)}</Alert>
      ) : (
        <Stack>
          <Text size='sm' c='dimmed'>
            {t(`${page}.description`, {})}
          </Text>

          {off && <Alert color='yellow'>{t(`${page}.alert.off`, {}).md()}</Alert>}

          {failovers.data?.failovers.length === 0 ? (
            <Text size='sm' c='dimmed'>
              {t(`${page}.empty`, {})}
            </Text>
          ) : (
            failovers.data?.failovers.map((failover) => (
              <TitleCard
                key={failover.uuid}
                title={t(`${page}.mode.${failover.mode}`, {})}
                rightSection={
                  <Group gap='xs'>
                    <Text size='sm' c='dimmed'>
                      <FormattedTimestamp timestamp={failover.created} />
                      {failover.startedBy && ` · ${failover.startedBy}`}
                    </Text>
                    <Badge color={failoverColors[failover.status]} variant='light'>
                      {t(`${page}.status.${failover.status}`, {})}
                    </Badge>
                  </Group>
                }
              >
                <Table
                  columns={[
                    t('common.table.columns.server', {}),
                    t('common.table.columns.status', {}),
                    t(`${page}.table.destination`, {}),
                    t(`${page}.table.backup`, {}),
                    t(`${page}.table.cleaned`, {}),
                  ]}
                >
                  {failover.servers.map((server) => (
                    <TableRow key={server.uuid}>
                      <TableData>
                        <TableLink to={`/admin/servers/${server.serverUuid}`}>
                          <Code>{server.serverName}</Code>
                        </TableLink>
                      </TableData>
                      <TableData>
                        <Badge color={serverColors[server.status]} variant='light'>
                          {t(`${page}.serverStatus.${server.status}`, {})}
                        </Badge>
                        {server.error && (
                          <Text size='xs' c='dimmed'>
                            {server.error}
                          </Text>
                        )}
                      </TableData>
                      <TableData>
                        {server.destinationNodeUuid && server.destinationNodeName ? (
                          <TableLink to={`/admin/nodes/${server.destinationNodeUuid}`}>
                            {server.destinationNodeName}
                          </TableLink>
                        ) : (
                          t('common.na', {})
                        )}
                      </TableData>
                      <TableData>
                        {server.backupCreated ? (
                          <FormattedTimestamp timestamp={server.backupCreated} />
                        ) : (
                          t('common.na', {})
                        )}
                      </TableData>
                      <TableData>
                        {server.sourceCleaned ? (
                          <FormattedTimestamp timestamp={server.sourceCleaned} />
                        ) : server.status === 'done' ? (
                          t(`${page}.cleanedPending`, {})
                        ) : (
                          t('common.na', {})
                        )}
                      </TableData>
                    </TableRow>
                  ))}
                </Table>
              </TitleCard>
            ))
          )}
        </Stack>
      )}
    </AdminSubContentContainer>
  );
}
