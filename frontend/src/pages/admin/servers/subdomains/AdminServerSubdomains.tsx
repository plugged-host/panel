// plugged.host: a server's subdomains and its subdomain limit in Admin › Servers (docs/features/subdomains.md)
import { faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import deleteServerSubdomain from '@/api/admin/servers/subdomains/deleteServerSubdomain.ts';
import getServerSubdomains from '@/api/admin/servers/subdomains/getServerSubdomains.ts';
import updateServerSubdomainLimit from '@/api/admin/servers/subdomains/updateServerSubdomainLimit.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminServer } from '@/lib/schemas/admin/servers.ts';
import { serverSubdomainSchema } from '@/lib/schemas/server/subdomains.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Subdomain = z.infer<typeof serverSubdomainSchema>;

const base = 'pages.admin.servers.tabs.subdomains';

function LimitForm({
  server,
  limit,
  defaultLimit,
}: {
  server: AdminServer;
  limit: number | null;
  defaultLimit: number;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [value, setValue] = useState<number | string>(limit ?? '');
  const [loading, setLoading] = useState(false);

  useEffect(() => setValue(limit ?? ''), [limit]);

  const save = () => {
    setLoading(true);
    updateServerSubdomainLimit(server.uuid, typeof value === 'number' ? value : null)
      .then(() => {
        addToast(t(`${base}.toast.limitSaved`, {}), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.servers.subdomains(server.uuid) });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Stack gap='sm' align='flex-start'>
      <NumberInput
        label={t(`${base}.limit`, {})}
        description={t(`${base}.limitDescription`, { default: defaultLimit })}
        placeholder={String(defaultLimit)}
        value={value}
        onChange={setValue}
        min={0}
        max={100}
        allowDecimal={false}
      />
      <Button onClick={save} loading={loading}>
        {t('common.button.save', {})}
      </Button>
    </Stack>
  );
}

export default function AdminServerSubdomains({ server }: { server: AdminServer }) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [deleting, setDeleting] = useState<Subdomain | null>(null);

  const resource = useResource({
    queryKey: queryKeys.admin.servers.subdomains(server.uuid),
    queryFn: () => getServerSubdomains(server.uuid),
  });

  const remove = async () => {
    if (!deleting) return;

    try {
      const refunded = await deleteServerSubdomain(server.uuid, deleting.uuid);
      addToast(
        refunded > 0
          ? t(`${base}.toast.deletedRefund`, { name: deleting.fqdn, count: refunded })
          : t(`${base}.toast.deleted`, { name: deleting.fqdn }),
        'success',
      );
      setDeleting(null);
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.servers.subdomains(server.uuid) });
    } catch (error) {
      addToast(httpErrorToHuman(error), 'error');
    }
  };

  return (
    <AdminSubContentContainer title={t(`${base}.title`, {})} titleOrder={2}>
      <ConfirmationModal
        opened={deleting !== null}
        onClose={() => setDeleting(null)}
        title={t('pages.server.subdomains.delete.title', {})}
        confirm={t('pages.server.subdomains.button.delete', {})}
        onConfirmed={remove}
      >
        <Stack gap='xs'>
          <Text size='sm'>{t('pages.server.subdomains.delete.body', { name: deleting?.fqdn ?? '' })}</Text>
          {deleting && deleting.tokenCost > 0 && (
            <Text size='sm' c='dimmed'>
              {t('pages.server.subdomains.delete.tokensBack', { tokens: tItem('token', deleting.tokenCost) })}
            </Text>
          )}
        </Stack>
      </ConfirmationModal>

      <ResourceView resource={resource}>
        {(overview) => (
          <Stack gap='lg'>
            <Table
              columns={[
                t(`${base}.subdomain`, {}),
                t('pages.server.subdomains.table.connect', {}),
                t('pages.server.subdomains.table.type', {}),
                t('pages.server.subdomains.table.deposit', {}),
                t(`${base}.created`, {}),
                '',
              ]}
              pagination={{
                total: overview.subdomains.length,
                perPage: Math.max(overview.subdomains.length, 1),
                page: 1,
                data: overview.subdomains,
              }}
              empty={
                <Text size='sm' c='dimmed'>
                  {t(`${base}.empty`, {})}
                </Text>
              }
            >
              {overview.subdomains.map((subdomain) => (
                <TableRow key={subdomain.uuid}>
                  <TableData>
                    <Text size='sm' fw={500}>
                      {subdomain.fqdn}
                    </Text>
                  </TableData>
                  <TableData>
                    <Code>{subdomain.connect}</Code>
                  </TableData>
                  <TableData>
                    <Text size='sm'>{subdomain.recordType}</Text>
                  </TableData>
                  <TableData>
                    <Text size='sm'>
                      {subdomain.tokenCost > 0 ? tItem('token', subdomain.tokenCost) : t(`${base}.free`, {})}
                    </Text>
                  </TableData>
                  <TableData>
                    <FormattedTimestamp timestamp={subdomain.created} />
                  </TableData>
                  <TableData className='text-end'>
                    <Group justify='flex-end'>
                      <Tooltip label={t('pages.server.subdomains.button.delete', {})}>
                        <ActionIcon
                          variant='default'
                          aria-label={t('pages.server.subdomains.button.delete', {})}
                          onClick={() => setDeleting(subdomain)}
                        >
                          <FontAwesomeIcon icon={faTrash} />
                        </ActionIcon>
                      </Tooltip>
                    </Group>
                  </TableData>
                </TableRow>
              ))}
            </Table>

            <LimitForm server={server} limit={overview.limit} defaultLimit={overview.defaultLimit} />
          </Stack>
        )}
      </ResourceView>
    </AdminSubContentContainer>
  );
}
