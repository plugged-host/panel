// plugged.host: Admin › Domains, the Cloudflare zones server subdomains are made under (docs/features/subdomains.md)
import { faRotate, faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Link } from 'react-router';
import { z } from 'zod';
import createDomain from '@/api/admin/domains/createDomain.ts';
import deleteDomain from '@/api/admin/domains/deleteDomain.ts';
import getDomains from '@/api/admin/domains/getDomains.ts';
import syncDomain from '@/api/admin/domains/syncDomain.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminDomainSchema } from '@/lib/schemas/admin/domains.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Domain = z.infer<typeof adminDomainSchema>;

const base = 'pages.admin.domains';

function CreateModal({ opened, onClose }: { opened: boolean; onClose: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [name, setName] = useState('');
  const [loading, setLoading] = useState(false);

  const close = () => {
    setName('');
    onClose();
  };

  const create = () => {
    setLoading(true);
    createDomain(name.trim())
      .then((domain) => {
        addToast(t(`${base}.toast.created`, { name: domain.name }), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.domains.all() });
        close();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal opened={opened} onClose={close} title={t(`${base}.create.title`, {})}>
      <TextInput
        label={t(`${base}.create.name`, {})}
        description={t(`${base}.create.nameDescription`, {})}
        placeholder='example.com'
        value={name}
        onChange={(e) => setName(e.currentTarget.value)}
        maxLength={253}
        data-autofocus
        required
      />

      <ModalFooter>
        <Button onClick={create} loading={loading} disabled={!name.trim().includes('.')}>
          {t(`${base}.create.button`, {})}
        </Button>
        <Button variant='default' onClick={close}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

function DomainRow({ domain, onDelete }: { domain: Domain; onDelete: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canUpdate = useAdminCan('domains.update');
  const canDelete = useAdminCan('domains.delete');
  const [syncing, setSyncing] = useState(false);

  const sync = () => {
    setSyncing(true);
    syncDomain(domain.uuid)
      .then(() => addToast(t(`${base}.toast.synced`, { name: domain.name }), 'success'))
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => {
        setSyncing(false);
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.domains.all() });
      });
  };

  return (
    <TableRow>
      <TableData>
        <Text size='sm' fw={500}>
          {domain.name}
        </Text>
      </TableData>
      <TableData>
        <Text size='sm' c={domain.synced ? undefined : 'red'}>
          {domain.synced ? t(`${base}.zone.linked`, {}) : t(`${base}.zone.missing`, {})}
        </Text>
      </TableData>
      <TableData>
        <Text size='sm'>{domain.subdomains}</Text>
      </TableData>
      <TableData>
        <FormattedTimestamp timestamp={domain.created} />
      </TableData>
      <TableData className='text-end'>
        <Group gap='xs' justify='flex-end' wrap='nowrap'>
          {canUpdate && (
            <Tooltip label={t(`${base}.button.sync`, {})}>
              <ActionIcon variant='default' aria-label={t(`${base}.button.sync`, {})} loading={syncing} onClick={sync}>
                <FontAwesomeIcon icon={faRotate} />
              </ActionIcon>
            </Tooltip>
          )}
          {canDelete && (
            <Tooltip label={t(`${base}.button.delete`, {})}>
              <ActionIcon variant='default' aria-label={t(`${base}.button.delete`, {})} onClick={onDelete}>
                <FontAwesomeIcon icon={faTrash} />
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
      </TableData>
    </TableRow>
  );
}

export default function AdminDomains() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canCreate = useAdminCan('domains.create');
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState<Domain | null>(null);

  const resource = useResource({
    queryKey: queryKeys.admin.domains.all(),
    queryFn: getDomains,
  });

  const remove = async () => {
    if (!deleting) return;

    try {
      await deleteDomain(deleting.uuid);
      addToast(t(`${base}.toast.deleted`, { name: deleting.name }), 'success');
      setDeleting(null);
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.domains.all() });
    } catch (error) {
      addToast(httpErrorToHuman(error), 'error');
    }
  };

  return (
    <AdminContentContainer
      title={t(`${base}.title`, {})}
      contentRight={
        canCreate && resource.data?.tokenSet ? (
          <Button onClick={() => setCreating(true)}>{t(`${base}.button.add`, {})}</Button>
        ) : undefined
      }
    >
      <CreateModal opened={creating} onClose={() => setCreating(false)} />
      <ConfirmationModal
        opened={deleting !== null}
        onClose={() => setDeleting(null)}
        title={t(`${base}.delete.title`, {})}
        confirm={t(`${base}.button.delete`, {})}
        onConfirmed={remove}
      >
        <Text size='sm'>
          {deleting && deleting.subdomains > 0
            ? t(`${base}.delete.bodyWithSubdomains`, { name: deleting.name, count: deleting.subdomains })
            : t(`${base}.delete.body`, { name: deleting?.name ?? '' })}
        </Text>
      </ConfirmationModal>

      <ResourceView resource={resource}>
        {(overview) => (
          <Stack gap='md'>
            <Text size='sm' c='dimmed'>
              {overview.tokenSet ? t(`${base}.description`, {}) : t(`${base}.noToken`, {})}{' '}
              <Link to='/admin/settings/hosting/subdomains' className='text-(--mantine-color-anchor) hover:underline'>
                {t(`${base}.settingsLink`, {})}
              </Link>
            </Text>

            <Table
              columns={[
                t(`${base}.table.name`, {}),
                t(`${base}.table.zone`, {}),
                t(`${base}.table.subdomains`, {}),
                t(`${base}.table.created`, {}),
                '',
              ]}
              pagination={{
                total: overview.domains.length,
                perPage: Math.max(overview.domains.length, 1),
                page: 1,
                data: overview.domains,
              }}
              empty={
                <Text size='sm' c='dimmed'>
                  {t(`${base}.empty`, {})}
                </Text>
              }
            >
              {overview.domains.map((domain) => (
                <DomainRow key={domain.uuid} domain={domain} onDelete={() => setDeleting(domain)} />
              ))}
            </Table>
          </Stack>
        )}
      </ResourceView>
    </AdminContentContainer>
  );
}
