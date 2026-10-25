// plugged.host: a failover's dry run: where each server of the node would go and from which backup,
// without moving anything (docs/features/failover.md).
import { ModalProps } from '@mantine/core';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import getNodeFailoverPlan from '@/api/admin/nodes/failover/getNodeFailoverPlan.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminFailoverPlanSchema } from '@/lib/schemas/admin/failover.ts';
import { adminNodeSchema } from '@/lib/schemas/admin/nodes.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Props = ModalProps & {
  node: z.infer<typeof adminNodeSchema>;
};

export default function FailoverPlanModal({ node, ...props }: Props) {
  const { t } = useTranslations();
  const page = 'pages.admin.nodes.tabs.failover.page';

  const [plan, setPlan] = useState<z.infer<typeof adminFailoverPlanSchema>[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!props.opened) {
      setPlan(null);
      setError(null);
      return;
    }

    // an answer that comes after the modal closed belongs to no preview
    let current = true;
    getNodeFailoverPlan(node.uuid)
      .then((plan) => current && setPlan(plan))
      .catch((err) => current && setError(httpErrorToHuman(err)));

    return () => {
      current = false;
    };
  }, [props.opened, node.uuid]);

  return (
    <Modal title={t(`${page}.modal.plan.title`, {})} size='xl' {...props}>
      <Stack>
        <Text size='sm' c='dimmed'>
          {t(`${page}.modal.plan.content`, {})}
        </Text>

        {error ? (
          <Alert color='red'>{error}</Alert>
        ) : !plan ? (
          <Spinner.Centered />
        ) : (
          <Table
            columns={[
              t('common.table.columns.server', {}),
              t(`${page}.table.destination`, {}),
              t(`${page}.table.backup`, {}),
            ]}
          >
            {plan.map((server) => (
              <TableRow key={server.serverUuid}>
                <TableData>
                  <TableLink to={`/admin/servers/${server.serverUuid}`}>
                    <Code>{server.serverName}</Code>
                  </TableLink>
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
                  {server.backupCreated ? <FormattedTimestamp timestamp={server.backupCreated} /> : t('common.na', {})}
                </TableData>
              </TableRow>
            ))}
          </Table>
        )}

        <ModalFooter>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
