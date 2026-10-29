// plugged.host: the server's subdomains, DNS names players join with, laid out like the Pelican
// panel's Subdomains page: a Filament table with its create action in the toolbar, and the create
// and edit modals (docs/features/subdomains.md)
import { faCopy, faGlobe, faPlus } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { FormEvent, ReactNode, useEffect, useState } from 'react';
import { ClipboardCancelledError } from 'shared';
import { z } from 'zod';
import { getHttpStatus, httpErrorToHuman } from '@/api/axios.ts';
import createSubdomain from '@/api/server/subdomains/createSubdomain.ts';
import deleteSubdomain from '@/api/server/subdomains/deleteSubdomain.ts';
import getSubdomains from '@/api/server/subdomains/getSubdomains.ts';
import updateSubdomain from '@/api/server/subdomains/updateSubdomain.ts';
import Button from '@/elements/buttons/Button.tsx';
import UnstyledButton from '@/elements/buttons/UnstyledButton.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { copyToClipboard } from '@/lib/clipboard/copy.ts';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverSubdomainSchema, serverSubdomainsSchema } from '@/lib/schemas/server/subdomains.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Overview = z.infer<typeof serverSubdomainsSchema>;
type Subdomain = z.infer<typeof serverSubdomainSchema>;
type Translate = ReturnType<typeof useTranslations>['t'];

const base = 'pages.server.subdomains';
const NAME = /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/;
/** How long new records show as processing (`PROPAGATION_MINUTES` in shared/src/subdomains). */
const PROPAGATION_MS = 3 * 60 * 1000;
/** The Pelican table polls every 15 s; the processing minutes count down at the same pace. */
const POLL_MS = 15000;

/** Minutes until a new subdomain counts as propagated, `null` once it does (Pelican's `status_label`). */
function processingMinutes(subdomain: Subdomain, now: number) {
  const left = Date.parse(subdomain.created) + PROPAGATION_MS - now;

  return subdomain.propagating && left > 0 ? Math.max(1, Math.ceil(left / 60000)) : null;
}

/** The Name field's helper text for the server's game (Pelican's `helper_*`). */
function serviceHelper(overview: Overview, port: number | undefined, t: Translate) {
  // a string, so the port is not printed with a thousands separator
  const portText = port === undefined ? 'port' : String(port);

  switch (overview.service) {
    case 'minecraft':
      return t(`${base}.create.service.minecraft`, { port: portText });
    case 'fivem':
      return t(`${base}.create.service.fivem`, { port: portText });
    case null:
    case 'plain':
      return t(`${base}.create.service.plain`, { address: overview.address ?? t(`${base}.create.serverIp`, {}) });
    default:
      return t(`${base}.create.service.srv`, {});
  }
}

/** Filament's copyable column: a click copies the value and says so in a tooltip for two seconds. */
function CopyCell({ value, message, children }: { value: string; message: string; children: ReactNode }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(timer);
  }, [copied]);

  const copy = () => {
    copyToClipboard(value)
      .then(() => setCopied(true))
      .catch((error) => {
        if (error instanceof ClipboardCancelledError) return;
        addToast(t('elements.copyOnClick.toast.failed', {}), 'error');
      });
  };

  return (
    <Tooltip label={message} opened={copied}>
      <UnstyledButton className='theme-copy-cell' onClick={copy}>
        {children}
      </UnstyledButton>
    </Tooltip>
  );
}

/** Pelican's `game_label` badge: green for Minecraft, yellow for FiveM / RedM, blue for the rest. */
function TypeBadge({ subdomain, service }: { subdomain: Subdomain; service: string | null }) {
  const { t } = useTranslations();

  switch (service) {
    case 'minecraft':
      return <Badge color='green'>{t(`${base}.type.minecraft`, {})}</Badge>;
    case 'fivem':
      return <Badge color='yellow'>{t(`${base}.type.fivem`, {})}</Badge>;
    case null:
    case 'plain':
      return <Badge color='blue'>{t(`${base}.type.standard`, { type: subdomain.recordType })}</Badge>;
    default:
      // Pelican names the other SRV services by their enum case in title case (Teamspeak, Scpsl).
      return (
        <Badge color='blue'>
          {t(`${base}.type.srv`, { service: service.charAt(0).toUpperCase() + service.slice(1) })}
        </Badge>
      );
  }
}

/** The deposit as Pelican shows it: the number large, its word beside it ("3 tokens", "3 токена"). */
function DepositAmount({ cost }: { cost: number }) {
  const { t, tItem, language } = useTranslations();
  const text = tItem('token', cost);
  const value = new Intl.NumberFormat(intlLocale(language)).format(cost);
  const at = text.indexOf(value);

  return (
    <div className='theme-sub-deposit-amount'>
      {at < 0 ? (
        <span>{text}</span>
      ) : (
        <>
          {text.slice(0, at).trim() && <span>{text.slice(0, at).trim()}</span>}
          <strong>{value}</strong>
          {text.slice(at + value.length).trim() && <span>{text.slice(at + value.length).trim()}</span>}
        </>
      )}
      <span>{t(`${base}.create.refundNote`, {})}</span>
    </div>
  );
}

/** The create form; without the deposit and with the domain fixed, the edit form. */
function SubdomainModal({
  opened,
  overview,
  editing,
  onClose,
}: {
  opened: boolean;
  overview: Overview;
  editing: Subdomain | null;
  onClose: () => void;
}) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const tokensEnabled = useGlobalStore((state) => state.settings.economy.tokensEnabled);
  const [name, setName] = useState('');
  const [domain, setDomain] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!opened) return;
    setName(editing?.name ?? '');
    setDomain(editing ? null : (overview.domains[0]?.uuid ?? null));
    setError(null);
  }, [opened]);

  const trimmed = name.trim().toLowerCase();
  // plugged.host: a name of 3 characters or fewer takes a bigger deposit (docs/features/server-upgrades.md)
  const shortCost = trimmed.length === 2 ? overview.shortTwo : trimmed.length === 3 ? overview.shortThree : 0;
  const cost = Math.max(overview.nextCost, shortCost);
  const domainName = editing ? editing.domain : (overview.domains.find((choice) => choice.uuid === domain)?.name ?? '');

  const submit = (e: FormEvent) => {
    e.preventDefault();

    if (trimmed.length < 2) {
      setError(t(`${base}.create.nameLength`, {}));
      return;
    }
    if (!NAME.test(trimmed)) {
      setError(t(`${base}.create.nameFormat`, {}));
      return;
    }
    if (!editing && cost > 0 && overview.balance !== null && overview.balance < cost) {
      setError(t(`${base}.create.insufficient`, { cost: tItem('token', cost), balance: overview.balance }));
      return;
    }

    setLoading(true);

    (editing ? updateSubdomain(server.uuid, editing.uuid, trimmed) : createSubdomain(server.uuid, trimmed, domain))
      .then((subdomain) => {
        addToast(
          editing ? t(`${base}.toast.saved`, {}) : t(`${base}.toast.created`, { name: subdomain.fqdn }),
          'success',
        );
        onClose();
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).subdomains() });
      })
      .catch((err) => {
        // What the form could have caught (a bad, blocked or taken name, too few tokens, the limit)
        // goes under the field, like Filament's validation; anything else is a toast.
        const status = getHttpStatus(err);
        if (status === 400 || status === 409) {
          setError(httpErrorToHuman(err));
        } else {
          addToast(httpErrorToHuman(err), 'error');
        }
      })
      .finally(() => setLoading(false));
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={editing ? t(`${base}.edit.title`, {}) : t(`${base}.create.title`, {})}
    >
      <form onSubmit={submit}>
        <Stack gap='lg'>
          {!editing && tokensEnabled && (
            <div className='theme-sub-deposit'>
              <div className='theme-sub-deposit-top'>
                <div>
                  <span className='theme-sub-deposit-caption'>
                    {shortCost > overview.nextCost
                      ? t(`${base}.create.shortCaption`, {})
                      : t(`${base}.create.depositCaption`, { number: overview.subdomains.length + 1 })}
                  </span>
                  <DepositAmount cost={cost} />
                </div>
                {/* Only the owner sees their balance; a subuser sees the price alone. */}
                {overview.balance !== null && (
                  <div className='theme-sub-deposit-balance'>
                    <span className='theme-sub-deposit-caption'>{t(`${base}.create.balance`, {})}</span>
                    <strong>{tItem('token', overview.balance)}</strong>
                  </div>
                )}
              </div>
            </div>
          )}

          <TextInput
            label={t(`${base}.create.name`, {})}
            placeholder={t(`${base}.create.placeholder`, {})}
            description={serviceHelper(overview, server.allocation?.port, t)}
            inputWrapperOrder={['label', 'input', 'description', 'error']}
            value={name}
            onChange={(e) => {
              setName(e.currentTarget.value);
              setError(null);
            }}
            error={error}
            rightSection={
              <Text size='sm' c='dimmed' pe='sm'>
                .{domainName}
              </Text>
            }
            rightSectionWidth={`${domainName.length + 3}ch`}
            maxLength={63}
            data-autofocus
            required
          />

          {overview.domains.length > 1 &&
            (editing ? (
              <Select
                label={t(`${base}.create.domain`, {})}
                data={[{ value: editing.domain, label: editing.domain }]}
                value={editing.domain}
                disabled
              />
            ) : (
              <Select
                label={t(`${base}.create.domain`, {})}
                data={overview.domains.map((choice) => ({ value: choice.uuid, label: choice.name }))}
                value={domain}
                onChange={setDomain}
                allowDeselect={false}
                required
              />
            ))}
        </Stack>

        <ModalFooter>
          <Button type='submit' loading={loading} disabled={trimmed === '' || (!editing && !domain)}>
            {editing ? t(`${base}.edit.button`, {}) : t(`${base}.create.button`, {})}
          </Button>
          <Button variant='default' onClick={onClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}

function SubdomainRow({
  subdomain,
  service,
  showDeposit,
  showActions,
  now,
  onEdit,
  onDelete,
}: {
  subdomain: Subdomain;
  service: string | null;
  showDeposit: boolean;
  showActions: boolean;
  now: number;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const canUpdate = useServerCan('subdomains.update');
  const canDelete = useServerCan('subdomains.delete');
  const [repairing, setRepairing] = useState(false);
  const minutes = processingMinutes(subdomain, now);

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
        <CopyCell value={subdomain.fqdn} message={t(`${base}.copied`, {})}>
          {subdomain.fqdn}
        </CopyCell>
      </TableData>
      <TableData>
        {minutes !== null ? (
          <Tooltip label={t(`${base}.status.processingTooltip`, {})}>
            <Badge color='yellow'>{t(`${base}.status.processing`, { minutes })}</Badge>
          </Tooltip>
        ) : (
          <Tooltip label={t(`${base}.status.activeTooltip`, {})}>
            <Badge color='green'>{t(`${base}.status.active`, {})}</Badge>
          </Tooltip>
        )}
      </TableData>
      <TableData>
        <TypeBadge subdomain={subdomain} service={service} />
      </TableData>
      {showDeposit && (
        <TableData>
          <Tooltip label={t(`${base}.depositTooltip`, {})}>
            <Badge color={subdomain.tokenCost > 0 ? 'yellow' : 'gray'}>{tItem('token', subdomain.tokenCost)}</Badge>
          </Tooltip>
        </TableData>
      )}
      <TableData>
        <CopyCell value={subdomain.connect} message={t(`${base}.connectionCopied`, {})}>
          <FontAwesomeIcon icon={faCopy} className='theme-copy-cell-icon' />
          {subdomain.connect}
        </CopyCell>
      </TableData>
      {showActions && (
        <TableData className='text-end'>
          <div className='theme-link-actions'>
            {canUpdate && (
              <>
                <UnstyledButton className='theme-link-action' disabled={repairing} onClick={repair}>
                  {t(`${base}.button.repair`, {})}
                </UnstyledButton>
                <UnstyledButton className='theme-link-action' onClick={onEdit}>
                  {t(`${base}.button.edit`, {})}
                </UnstyledButton>
              </>
            )}
            {canDelete && (
              <UnstyledButton className='theme-link-action' data-danger onClick={onDelete}>
                {t(`${base}.button.delete`, {})}
              </UnstyledButton>
            )}
          </div>
        </TableData>
      )}
    </TableRow>
  );
}

function Subdomains({ overview }: { overview: Overview }) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const tokensEnabled = useGlobalStore((state) => state.settings.economy.tokensEnabled);
  const canCreate = useServerCan('subdomains.create');
  const canUpdate = useServerCan('subdomains.update');
  const canDelete = useServerCan('subdomains.delete');
  const [creating, setCreating] = useState(false);
  // The subdomain stays set while its modal closes, so the modal does not change under the animation.
  const [editing, setEditing] = useState<Subdomain | null>(null);
  const [editOpened, setEditOpened] = useState(false);
  const [deleting, setDeleting] = useState<Subdomain | null>(null);
  const [deleteOpened, setDeleteOpened] = useState(false);
  const [now, setNow] = useState(() => Date.now());

  // Counts the processing minutes down while a subdomain is new.
  const processing = overview.subdomains.some((subdomain) => subdomain.propagating);
  useEffect(() => {
    if (!processing) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), POLL_MS);
    return () => clearInterval(timer);
  }, [processing]);

  const remove = async () => {
    if (!deleting) return;

    const refunded = await deleteSubdomain(server.uuid, deleting.uuid);
    addToast(
      refunded > 0
        ? t(`${base}.toast.deletedWithRefund`, { tokens: tItem('token', refunded) })
        : t(`${base}.toast.deleted`, {}),
      'success',
    );
    setDeleteOpened(false);
    queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).subdomains() });
  };

  // The Pelican panel hides the page while no subdomain can be made; here the reason takes the
  // table's place, and a server that still has subdomains keeps them listed.
  if (overview.unavailable && overview.subdomains.length === 0) {
    return (
      <EmptyState
        flush
        icon={faGlobe}
        title={t(`${base}.unavailable.${overview.unavailable}`, {})}
        description={null}
      />
    );
  }

  const full = overview.subdomains.length >= overview.limit;
  const createTooltip = overview.unavailable
    ? t(`${base}.unavailable.${overview.unavailable}`, {})
    : full
      ? t(`${base}.limitReached`, {})
      : t(`${base}.button.add`, {});
  const showActions = canUpdate || canDelete;

  return (
    <div>
      <SubdomainModal opened={creating} overview={overview} editing={null} onClose={() => setCreating(false)} />
      <SubdomainModal opened={editOpened} overview={overview} editing={editing} onClose={() => setEditOpened(false)} />
      <ConfirmationModal
        opened={deleteOpened}
        onClose={() => setDeleteOpened(false)}
        title={t(`${base}.delete.title`, {})}
        confirm={t(`${base}.button.delete`, {})}
        onConfirmed={remove}
      >
        <Stack gap='xs'>
          <Text size='sm'>{t(`${base}.delete.body`, { name: deleting?.fqdn ?? '' })}</Text>
          {deleting && deleting.tokenCost > 0 && (
            <Text size='sm' c='dimmed'>
              {t(`${base}.delete.tokensBack`, { tokens: tItem('token', deleting.tokenCost) })}
            </Text>
          )}
        </Stack>
      </ConfirmationModal>

      {/* Filament's table toolbar: the create action as an icon at its start, over a hairline. */}
      {canCreate && (
        <div className='theme-table-toolbar'>
          <div className='theme-table-toolbar-actions'>
            <Tooltip label={createTooltip}>
              <UnstyledButton
                className='theme-toolbar-action'
                aria-label={t(`${base}.button.add`, {})}
                disabled={!!overview.unavailable || full}
                onClick={() => setCreating(true)}
              >
                <FontAwesomeIcon icon={faPlus} />
              </UnstyledButton>
            </Tooltip>
          </div>
        </div>
      )}

      <Table
        columns={[
          t(`${base}.table.name`, {}),
          t(`${base}.table.status`, {}),
          t(`${base}.table.type`, {}),
          ...(tokensEnabled ? [t(`${base}.table.deposit`, {})] : []),
          t(`${base}.table.connect`, {}),
          ...(showActions ? [''] : []),
        ]}
        pagination={{
          total: overview.subdomains.length,
          perPage: Math.max(overview.subdomains.length, 1),
          page: 1,
          data: overview.subdomains,
        }}
        empty={<EmptyState flush icon={faGlobe} title={t(`${base}.empty`, {})} description={null} />}
      >
        {overview.subdomains.map((subdomain) => (
          <SubdomainRow
            key={subdomain.uuid}
            subdomain={subdomain}
            service={overview.service}
            showDeposit={tokensEnabled}
            showActions={showActions}
            now={now}
            onEdit={() => {
              setEditing(subdomain);
              setEditOpened(true);
            }}
            onDelete={() => {
              setDeleting(subdomain);
              setDeleteOpened(true);
            }}
          />
        ))}
      </Table>
    </div>
  );
}

export default function ServerSubdomains() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const query = useQuery({
    queryKey: queryKeys.server(server.uuid).subdomains(),
    queryFn: () => getSubdomains(server.uuid),
    // new records show as processing for a few minutes; the Pelican table polls every 15 s
    refetchInterval: (current) =>
      current.state.data?.subdomains.some((subdomain) => subdomain.propagating) ? POLL_MS : false,
  });
  const overview = query.data;

  return (
    <ServerContentContainer
      title={t(`${base}.title`, {})}
      subtitle={
        overview ? t(`${base}.subtitle`, { current: overview.subdomains.length, max: overview.limit }) : undefined
      }
    >
      <ResourceView
        resource={{ data: query.data, loading: query.isLoading, error: query.error, refetch: () => query.refetch() }}
      >
        {(overview) => <Subdomains overview={overview} />}
      </ResourceView>
    </ServerContentContainer>
  );
}
