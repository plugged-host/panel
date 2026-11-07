// plugged.host: Admin › Tickets (docs/features/tickets.md): filter by status and category, open
// the thread, reply as staff, assign self and change status or priority.
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import assignTicket from '@/api/admin/tickets/assignTicket.ts';
import deleteTicket from '@/api/admin/tickets/deleteTicket.ts';
import getTicket from '@/api/admin/tickets/getTicket.ts';
import getTickets from '@/api/admin/tickets/getTickets.ts';
import replyTicket from '@/api/admin/tickets/replyTicket.ts';
import updateTicket from '@/api/admin/tickets/updateTicket.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import UnstyledButton from '@/elements/buttons/UnstyledButton.tsx';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Select from '@/elements/input/Select.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { Modal } from '@/elements/modals/Modal.tsx';
import NeoMarkdown from '@/elements/neo/NeoMarkdown.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { type TicketStatus, ticketStatus, ticketSummarySchema } from '@/lib/schemas/user/tickets.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Ticket = z.infer<typeof ticketSummarySchema>;

const base = 'pages.admin.tickets';

const statusColor: Record<TicketStatus, string> = {
  open: 'blue',
  waiting: 'yellow',
  resolved: 'green',
  closed: 'gray',
};

export function ThreadModal({
  ticket,
  onClose,
  inline = false,
}: {
  ticket: { uuid: string; subject?: string } | null;
  onClose: () => void;
  inline?: boolean;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canManage = useAdminCan('tickets.manage');
  const [thread, setThread] = useState<Awaited<ReturnType<typeof getTicket>> | null>(null);
  const [loading, setLoading] = useState(false);
  const [reply, setReply] = useState('');
  const [sending, setSending] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const load = (silent = false) => {
    if (!ticket) return;
    if (!silent) setLoading(true);
    getTicket(ticket.uuid)
      .then(setThread)
      .catch((error) => {
        if (!silent) addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => {
        if (!silent) setLoading(false);
      });
  };

  useEffect(() => {
    if (!ticket) {
      setThread(null);
      setReply('');
      return;
    }
    load();
    const timer = window.setInterval(() => load(true), 2_000);
    return () => window.clearInterval(timer);
  }, [ticket?.uuid]);

  const refresh = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.admin.tickets.all() });
    load();
  };

  const send = () => {
    if (!ticket || !reply.trim()) return;
    setSending(true);
    replyTicket(ticket.uuid, reply.trim())
      .then((next) => {
        setThread(next);
        setReply('');
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.tickets.all() });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSending(false));
  };

  const update = (payload: { status?: string; priority?: string }) => {
    if (!ticket) return;
    updateTicket(ticket.uuid, payload)
      .then((next) => {
        setThread(next);
        refresh();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'));
  };

  const assign = () => {
    if (!ticket) return;
    assignTicket(ticket.uuid, !thread?.ticket.assignedTo)
      .then((next) => {
        setThread(next);
        refresh();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'));
  };

  const content = (
    <>
      {loading && !thread ? (
        <Text size='sm' c='dimmed'>
          {t('common.loading', {})}
        </Text>
      ) : thread ? (
        <Stack gap='md'>
          <div className='flex flex-wrap items-center gap-2'>
            <Badge color={statusColor[thread.ticket.status]}>
              {t(`pages.account.tickets.status.${thread.ticket.status}`, {})}
            </Badge>
            <Badge color='gray' variant='light'>
              {t(`pages.account.tickets.category.${thread.ticket.category}`, {})}
            </Badge>
            <Text size='sm' c='dimmed'>
              {thread.ticket.user?.username}
            </Text>
            {thread.ticket.server && (
              <Text size='sm' c='dimmed'>
                {thread.ticket.server.name}
              </Text>
            )}
            <Text size='sm' c='dimmed'>
              {thread.ticket.assignedTo
                ? t(`${base}.thread.assigned`, { name: thread.ticket.assignedTo.username })
                : t(`${base}.thread.unassigned`, {})}
            </Text>
          </div>

          {thread.ticket.bug && (
            <div className='theme-eco-list'>
              <div className='theme-eco-tx'>
                <div className='flex min-w-0 flex-col gap-1'>
                  <span className='theme-eco-small'>{t('pages.account.tickets.thread.bug', {})}</span>
                  {thread.ticket.bug.steps && (
                    <p className='theme-eco-text'>
                      <strong>{t('pages.account.tickets.thread.bugSteps', {})}:</strong> {thread.ticket.bug.steps}
                    </p>
                  )}
                  {thread.ticket.bug.expected && (
                    <p className='theme-eco-text'>
                      <strong>{t('pages.account.tickets.thread.bugExpected', {})}:</strong> {thread.ticket.bug.expected}
                    </p>
                  )}
                  {thread.ticket.bug.actual && (
                    <p className='theme-eco-text'>
                      <strong>{t('pages.account.tickets.thread.bugActual', {})}:</strong> {thread.ticket.bug.actual}
                    </p>
                  )}
                  {thread.ticket.bug.environment && (
                    <p className='theme-eco-text'>
                      <strong>{t('pages.account.tickets.thread.bugEnvironment', {})}:</strong>{' '}
                      {thread.ticket.bug.environment}
                    </p>
                  )}
                </div>
              </div>
            </div>
          )}

          {thread.attachments.length > 0 && (
            <div className='flex flex-col gap-2'>
              <span className='theme-eco-small'>{t('pages.account.tickets.thread.attachments', {})}</span>
              <div className='flex flex-wrap gap-2'>
                {thread.attachments.map((attachment) =>
                  attachment.mime.startsWith('image/') ? (
                    <a key={attachment.uuid} href={attachment.url} target='_blank' rel='noopener noreferrer'>
                      <img
                        src={attachment.url}
                        alt={attachment.name}
                        className='max-h-48 rounded-lg border border-(--theme-hairline) object-contain'
                      />
                    </a>
                  ) : (
                    <a
                      key={attachment.uuid}
                      href={attachment.url}
                      target='_blank'
                      rel='noopener noreferrer'
                      className='theme-link-action'
                    >
                      {attachment.name}
                    </a>
                  ),
                )}
              </div>
            </div>
          )}

          <div className='theme-eco-list'>
            {thread.messages.map((message) => (
              <div key={message.uuid} className='theme-eco-tx'>
                <div className='flex min-w-0 flex-col'>
                  <span className='theme-eco-small'>
                    {message.authorType === 'neo'
                      ? t('pages.account.tickets.thread.neo', {})
                      : message.authorType === 'system'
                        ? t('pages.account.tickets.thread.system', {})
                        : message.authorType === 'admin'
                          ? t('pages.account.tickets.thread.staff', {})
                          : (message.author?.username ?? t('pages.account.tickets.thread.you', {}))}
                    {' · '}
                    <FormattedTimestamp timestamp={message.created} />
                  </span>
                  <p className='whitespace-pre-wrap theme-eco-text'>
                    {message.body || (message.authorType === 'neo' ? t(`${base}.thread.thinking`, {}) : '')}
                  </p>
                </div>
              </div>
            ))}
          </div>

          {canManage && (
            <>
              <div className='flex flex-wrap items-end gap-2'>
                <Select
                  label={t(`${base}.thread.status`, {})}
                  data={(['open', 'waiting', 'resolved', 'closed'] as const).map((status) => ({
                    value: status,
                    label: t(`pages.account.tickets.status.${status}`, {}),
                  }))}
                  value={thread.ticket.status}
                  onChange={(value) => value && update({ status: value })}
                  allowDeselect={false}
                  w={180}
                />
                <Select
                  label={t(`${base}.thread.priority`, {})}
                  data={(['low', 'normal', 'high'] as const).map((priority) => ({
                    value: priority,
                    label: t(`${base}.priority.${priority}`, {}),
                  }))}
                  value={thread.ticket.priority}
                  onChange={(value) => value && update({ priority: value })}
                  allowDeselect={false}
                  w={160}
                />
                <Button variant='default' onClick={assign}>
                  {thread.ticket.assignedTo ? t(`${base}.thread.unassign`, {}) : t(`${base}.thread.assign`, {})}
                </Button>
              </div>

              <TextArea
                autosize
                minRows={3}
                maxRows={8}
                label={t(`${base}.thread.reply`, {})}
                value={reply}
                onChange={(event) => setReply(event.currentTarget.value)}
              />
              <div className='flex flex-wrap gap-2'>
                <Button loading={sending} disabled={!reply.trim()} onClick={send}>
                  {t(`${base}.thread.send`, {})}
                </Button>
                <Button color='red' variant='outline' onClick={() => setConfirmDelete(true)}>
                  {t('common.button.delete', {})}
                </Button>
              </div>
            </>
          )}

          <ConfirmationModal
            opened={confirmDelete}
            onClose={() => setConfirmDelete(false)}
            title={t(`${base}.thread.deleteTitle`, {})}
            confirm={t('common.button.delete', {})}
            confirmColor='red'
            onConfirmed={() => {
              if (!ticket) return;
              deleteTicket(ticket.uuid)
                .then(() => {
                  queryClient.invalidateQueries({ queryKey: queryKeys.admin.tickets.all() });
                  onClose();
                })
                .catch((error) => addToast(httpErrorToHuman(error), 'error'));
            }}
          >
            {t(`${base}.thread.deleteBody`, {})}
          </ConfirmationModal>
        </Stack>
      ) : null}
    </>
  );

  if (inline) return content;

  return (
    <Modal
      opened={ticket !== null}
      onClose={() => {
        setThread(null);
        setReply('');
        onClose();
      }}
      title={ticket?.subject ?? t(`${base}.thread.title`, {})}
      size='xl'
    >
      {content}
    </Modal>
  );
}

export default function AdminTickets() {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<TicketStatus | null>(null);
  const [category, setCategory] = useState<string | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    const timer = window.setInterval(
      () => queryClient.invalidateQueries({ queryKey: queryKeys.admin.tickets.all() }),
      10_000,
    );
    return () => window.clearInterval(timer);
  }, [queryClient]);

  const { data, loading, error, search, setSearch, setPage, setPerPage } = useSearchablePaginatedTable({
    queryKey: [...queryKeys.admin.tickets.all(), { status, category }],
    fetcher: (page, search, perPage) => getTickets(page, search, perPage, status, category),
    deps: [status, category],
  });

  return (
    <AdminContentContainer
      title={t(`${base}.title`, {})}
      search={search}
      setSearch={setSearch}
      contentRight={
        <div className='flex flex-wrap gap-2'>
          <Select
            placeholder={t(`${base}.filter.status`, {})}
            value={status}
            onChange={(value) => setStatus(value as TicketStatus | null)}
            data={(['open', 'waiting', 'resolved', 'closed'] as const).map((value) => ({
              value,
              label: t(`pages.account.tickets.status.${value}`, {}),
            }))}
            clearable
            w={160}
          />
          <Select
            placeholder={t(`${base}.filter.category`, {})}
            value={category}
            onChange={setCategory}
            data={(['general', 'billing', 'technical', 'bug', 'abuse', 'other'] as const).map((value) => ({
              value,
              label: t(`pages.account.tickets.category.${value}`, {}),
            }))}
            clearable
            w={160}
          />
        </div>
      }
    >
      <Table
        columns={[
          t(`${base}.table.subject`, {}),
          t(`${base}.table.user`, {}),
          t(`${base}.table.category`, {}),
          t(`${base}.table.status`, {}),
          t(`${base}.table.assigned`, {}),
          t(`${base}.table.updated`, {}),
          '',
        ]}
        loading={loading}
        error={error}
        pagination={data}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {data?.data.map((ticket) => (
          <TableRow key={ticket.uuid}>
            <TableData>
              <UnstyledButton className='theme-link-action' onClick={() => navigate(`/admin/tickets/${ticket.uuid}`)}>
                {ticket.subject}
              </UnstyledButton>
            </TableData>
            <TableData>{ticket.user?.username}</TableData>
            <TableData>{t(`pages.account.tickets.category.${ticket.category}`, {})}</TableData>
            <TableData>
              <Badge color={statusColor[ticket.status]}>{t(`pages.account.tickets.status.${ticket.status}`, {})}</Badge>
            </TableData>
            <TableData>{ticket.assignedTo?.username ?? t(`${base}.thread.unassigned`, {})}</TableData>
            <TableData>
              <FormattedTimestamp timestamp={ticket.updated} />
            </TableData>
            <TableData className='text-end'>
              <UnstyledButton className='theme-link-action' onClick={() => navigate(`/admin/tickets/${ticket.uuid}`)}>
                {t('common.button.view', {})}
              </UnstyledButton>
            </TableData>
          </TableRow>
        ))}
      </Table>
    </AdminContentContainer>
  );
}
