// plugged.host: Admin › Tickets (docs/features/tickets.md): filter by status and category, open
// the thread, reply as staff, assign self and change status or priority.
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import assignTicket from '@/api/admin/tickets/assignTicket.ts';
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
import { Modal } from '@/elements/modals/Modal.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { ticketSummarySchema, ticketStatus, type TicketStatus } from '@/lib/schemas/user/tickets.ts';
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

function ThreadModal({ ticket, onClose }: { ticket: Ticket | null; onClose: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canManage = useAdminCan('tickets.manage');
  const [thread, setThread] = useState<Awaited<ReturnType<typeof getTicket>> | null>(null);
  const [loading, setLoading] = useState(false);
  const [reply, setReply] = useState('');
  const [sending, setSending] = useState(false);

  const load = () => {
    if (!ticket) return;
    setLoading(true);
    getTicket(ticket.uuid)
      .then(setThread)
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    if (!ticket) {
      setThread(null);
      setReply('');
      return;
    }
    load();
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
            {thread.ticket.server && <Text size='sm' c='dimmed'>{thread.ticket.server.name}</Text>}
            <Text size='sm' c='dimmed'>
              {thread.ticket.assignedTo
                ? t(`${base}.thread.assigned`, { name: thread.ticket.assignedTo.username })
                : t(`${base}.thread.unassigned`, {})}
            </Text>
          </div>

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
                  <p className='whitespace-pre-wrap theme-eco-text'>{message.body}</p>
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
              <div>
                <Button loading={sending} disabled={!reply.trim()} onClick={send}>
                  {t(`${base}.thread.send`, {})}
                </Button>
              </div>
            </>
          )}
        </Stack>
      ) : null}
    </Modal>
  );
}

export default function AdminTickets() {
  const { t } = useTranslations();
  const [status, setStatus] = useState<TicketStatus | null>(null);
  const [category, setCategory] = useState<string | null>(null);
  const [opening, setOpening] = useState<Ticket | null>(null);

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
            data={(['general', 'billing', 'technical', 'abuse', 'other'] as const).map((value) => ({
              value,
              label: t(`pages.account.tickets.category.${value}`, {}),
            }))}
            clearable
            w={160}
          />
        </div>
      }
    >
      <ThreadModal ticket={opening} onClose={() => setOpening(null)} />

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
              <UnstyledButton className='theme-link-action' onClick={() => setOpening(ticket)}>
                {ticket.subject}
              </UnstyledButton>
            </TableData>
            <TableData>{ticket.user?.username}</TableData>
            <TableData>{t(`pages.account.tickets.category.${ticket.category}`, {})}</TableData>
            <TableData>
              <Badge color={statusColor[ticket.status]}>
                {t(`pages.account.tickets.status.${ticket.status}`, {})}
              </Badge>
            </TableData>
            <TableData>{ticket.assignedTo?.username ?? t(`${base}.thread.unassigned`, {})}</TableData>
            <TableData>
              <FormattedTimestamp timestamp={ticket.updated} />
            </TableData>
            <TableData className='text-end'>
              <UnstyledButton className='theme-link-action' onClick={() => setOpening(ticket)}>
                {t('common.button.view', {})}
              </UnstyledButton>
            </TableData>
          </TableRow>
        ))}
      </Table>
    </AdminContentContainer>
  );
}
