// plugged.host: the server's subdomains, DNS names players join with (docs/features/subdomains.md)
import { faPen, faRotate, faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createSubdomain from '@/api/server/subdomains/createSubdomain.ts';
import deleteSubdomain from '@/api/server/subdomains/deleteSubdomain.ts';
import getSubdomains from '@/api/server/subdomains/getSubdomains.ts';
import updateSubdomain from '@/api/server/subdomains/updateSubdomain.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import ConditionalTooltip from '@/elements/overlays/ConditionalTooltip.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverSubdomainSchema, serverSubdomainsSchema } from '@/lib/schemas/server/subdomains.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Overview = z.infer<typeof serverSubdomainsSchema>;
type Subdomain = z.infer<typeof serverSubdomainSchema>;

const base = 'pages.server.subdomains';
const NAME = /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/;

const validName = (name: string) => name.length >= 2 && NAME.test(name);

function serviceNote(overview: Overview, t: ReturnType<typeof useTranslations>['t']) {
  switch (overview.service) {
    case 'minecraft':
      return t(`${base}.create.service.minecraft`, {});
    case 'fivem':
      return t(`${base}.create.service.fivem`, {});
    case 'plain':
      return t(`${base}.create.service.plain`, { address: overview.address ?? '' });
    default:
      return t(`${base}.create.service.srv`, {});
  }
}

function CreateModal({ opened, overview, onClose }: { opened: boolean; overview: Overview; onClose: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const [name, setName] = useState('');
  const [domain, setDomain] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!opened) return;
    setName('');
    setDomain(overview.domains[0]?.uuid ?? null);
  }, [opened]);

  const trimmed = name.trim().toLowerCase();
  const domainName = overview.domains.find((choice) => choice.uuid === domain)?.name ?? '';
  const short = overview.balance !== null && overview.balance < overview.nextCost;

  const create = () => {
    setLoading(true);

    createSubdomain(server.uuid, trimmed, domain)
      .then((subdomain) => {
        addToast(t(`${base}.toast.created`, { name: subdomain.fqdn }), 'success');
        onClose();
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).subdomains() });
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <Modal opened={opened} onClose={onClose} title={t(`${base}.create.title`, {})}>
      <Stack gap='md'>
        <TextInput
          label={t(`${base}.create.name`, {})}
          description={t(`${base}.create.nameDescription`, {})}
          placeholder='play'
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          error={trimmed !== '' && !validName(trimmed)}
          rightSection={
            overview.domains.length === 1 ? (
              <Text size='sm' c='dimmed' pe='sm'>
                .{domainName}
              </Text>
            ) : undefined
          }
          rightSectionWidth={overview.domains.length === 1 ? `${domainName.length + 3}ch` : undefined}
          maxLength={63}
          data-autofocus
          required
        />
        {overview.domains.length > 1 && (
          <Select
            label={t(`${base}.create.domain`, {})}
            data={overview.domains.map((choice) => ({ value: choice.uuid, label: choice.name }))}
            value={domain}
            onChange={setDomain}
            allowDeselect={false}
            required
          />
        )}
        <Text size='sm' c='dimmed'>
          {serviceNote(overview, t)}
        </Text>
        {overview.nextCost > 0 && (
          <Text size='sm' c={short ? 'red' : undefined}>
            {short
              ? t(`${base}.create.notEnough`, { cost: overview.nextCost, balance: overview.balance ?? 0 })
              : t(`${base}.create.cost`, { cost: overview.nextCost })}
            {!short && overview.balance !== null && ` ${t(`${base}.create.balance`, { balance: overview.balance })}`}
          </Text>
        )}
      </Stack>

      <ModalFooter>
        <Button onClick={create} loading={loading} disabled={!validName(trimmed) || !domain || short}>
          {t(`${base}.create.button`, {})}
        </Button>
        <Button variant='default' onClick={onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

function RenameModal({ subdomain, onClose }: { subdomain: Subdomain | null; onClose: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const [name, setName] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (subdomain) setName(subdomain.name);
  }, [subdomain]);

  const trimmed = name.trim().toLowerCase();

  const rename = () => {
    if (!subdomain) return;
    setLoading(true);

    updateSubdomain(server.uuid, subdomain.uuid, trimmed)
      .then((renamed) => {
        addToast(t(`${base}.toast.renamed`, { name: renamed.fqdn }), 'success');
        onClose();
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).subdomains() });
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <Modal
      opened={subdomain !== null}
      onClose={onClose}
      title={t(`${base}.rename.title`, { name: subdomain?.fqdn ?? '' })}
    >
      <TextInput
        label={t(`${base}.create.name`, {})}
        description={t(`${base}.create.nameDescription`, {})}
        value={name}
        onChange={(e) => setName(e.currentTarget.value)}
        error={trimmed !== '' && !validName(trimmed)}
        rightSection={
          <Text size='sm' c='dimmed' pe='sm'>
            .{subdomain?.domain ?? ''}
          </Text>
        }
        rightSectionWidth={`${(subdomain?.domain.length ?? 0) + 3}ch`}
        maxLength={63}
        data-autofocus
        required
      />

      <ModalFooter>
        <Button onClick={rename} loading={loading} disabled={!validName(trimmed) || trimmed === subdomain?.name}>
          {t(`${base}.rename.button`, {})}
        </Button>
        <Button variant='default' onClick={onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

function SubdomainRow({
  subdomain,
  onRename,
  onDelete,
}: {
  subdomain: Subdomain;
  onRename: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const canUpdate = useServerCan('subdomains.update');
  const canDelete = useServerCan('subdomains.delete');
  const [repairing, setRepairing] = useState(false);

  const repair = () => {
    setRepairing(true);
    updateSubdomain(server.uuid, subdomain.uuid, null)
      .then(() => {
        addToast(t(`${base}.toast.repaired`, { name: subdomain.fqdn }), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).subdomains() });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setRepairing(false));
  };

  return (
    <TableRow>
      <TableData>
        <CopyOnClick content={subdomain.fqdn}>
          <Text size='sm' fw={500}>
            {subdomain.fqdn}
          </Text>
        </CopyOnClick>
      </TableData>
      <TableData>
        <CopyOnClick content={subdomain.connect}>
          <Code>{subdomain.connect}</Code>
        </CopyOnClick>
      </TableData>
      <TableData>
        <Text size='sm'>{subdomain.recordType}</Text>
      </TableData>
      <TableData>
        <Text size='sm'>
          {subdomain.tokenCost > 0 ? t(`${base}.deposit`, { count: subdomain.tokenCost }) : t(`${base}.free`, {})}
        </Text>
      </TableData>
      <TableData>
        {subdomain.propagating ? (
          <Tooltip label={t(`${base}.status.propagatingTooltip`, {})}>
            <Text size='sm' c='dimmed'>
              {t(`${base}.status.propagating`, {})}
            </Text>
          </Tooltip>
        ) : (
          <Text size='sm'>{t(`${base}.status.active`, {})}</Text>
        )}
      </TableData>
      <TableData className='text-end'>
        <Group gap='xs' justify='flex-end' wrap='nowrap'>
          {canUpdate && (
            <>
              <Tooltip label={t(`${base}.button.repair`, {})}>
                <ActionIcon
                  variant='default'
                  aria-label={t(`${base}.button.repair`, {})}
                  loading={repairing}
                  onClick={repair}
                >
                  <FontAwesomeIcon icon={faRotate} />
                </ActionIcon>
              </Tooltip>
              <Tooltip label={t(`${base}.button.rename`, {})}>
                <ActionIcon variant='default' aria-label={t(`${base}.button.rename`, {})} onClick={onRename}>
                  <FontAwesomeIcon icon={faPen} />
                </ActionIcon>
              </Tooltip>
            </>
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

function Subdomains({ overview }: { overview: Overview }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const [renaming, setRenaming] = useState<Subdomain | null>(null);
  const [deleting, setDeleting] = useState<Subdomain | null>(null);

  const remove = async () => {
    if (!deleting) return;

    try {
      const refunded = await deleteSubdomain(server.uuid, deleting.uuid);
      addToast(
        refunded > 0
          ? t(`${base}.toast.deletedRefund`, { name: deleting.fqdn, count: refunded })
          : t(`${base}.toast.deleted`, { name: deleting.fqdn }),
        'success',
      );
      setDeleting(null);
      queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).subdomains() });
    } catch (error) {
      addToast(httpErrorToHuman(error), 'error');
    }
  };

  return (
    <Stack gap='md'>
      <RenameModal subdomain={renaming} onClose={() => setRenaming(null)} />
      <ConfirmationModal
        opened={deleting !== null}
        onClose={() => setDeleting(null)}
        title={t(`${base}.delete.title`, {})}
        confirm={t(`${base}.button.delete`, {})}
        onConfirmed={remove}
      >
        <Stack gap='xs'>
          <Text size='sm'>{t(`${base}.delete.body`, { name: deleting?.fqdn ?? '' })}</Text>
          {deleting && deleting.tokenCost > 0 && (
            <Text size='sm' c='dimmed'>
              {t(`${base}.delete.refund`, { count: deleting.tokenCost })}
            </Text>
          )}
        </Stack>
      </ConfirmationModal>

      {overview.unavailable && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.unavailable.${overview.unavailable}`, {})}
        </Text>
      )}

      <Table
        columns={[
          t(`${base}.table.subdomain`, {}),
          t(`${base}.table.connect`, {}),
          t(`${base}.table.type`, {}),
          t(`${base}.table.deposit`, {}),
          t(`${base}.table.status`, {}),
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
          <SubdomainRow
            key={subdomain.uuid}
            subdomain={subdomain}
            onRename={() => setRenaming(subdomain)}
            onDelete={() => setDeleting(subdomain)}
          />
        ))}
      </Table>
    </Stack>
  );
}

export default function ServerSubdomains() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const canCreate = useServerCan('subdomains.create');
  const [creating, setCreating] = useState(false);

  const query = useQuery({
    queryKey: queryKeys.server(server.uuid).subdomains(),
    queryFn: () => getSubdomains(server.uuid),
    // new records show as spreading for a few minutes
    refetchInterval: (current) =>
      current.state.data?.subdomains.some((subdomain) => subdomain.propagating) ? 30000 : false,
  });
  const overview = query.data;
  const full = !!overview && overview.subdomains.length >= overview.limit;
  const blocked = !overview || !!overview.unavailable || full;

  return (
    <ServerContentContainer
      title={t(`${base}.title`, {})}
      subtitle={
        overview ? t(`${base}.subtitle`, { current: overview.subdomains.length, max: overview.limit }) : undefined
      }
      contentRight={
        canCreate && overview && !overview.unavailable ? (
          <ConditionalTooltip enabled={full} label={t(`${base}.full`, { max: overview.limit })}>
            <Button disabled={blocked} onClick={() => setCreating(true)}>
              {t(`${base}.button.add`, {})}
            </Button>
          </ConditionalTooltip>
        ) : undefined
      }
    >
      <ResourceView
        resource={{ data: query.data, loading: query.isLoading, error: query.error, refetch: () => query.refetch() }}
      >
        {(overview) => (
          <>
            <CreateModal opened={creating} overview={overview} onClose={() => setCreating(false)} />
            <Subdomains overview={overview} />
          </>
        )}
      </ResourceView>
    </ServerContentContainer>
  );
}
