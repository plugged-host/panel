// plugged.host: Account › Support, the account's tickets (docs/features/tickets.md). A full-width
// list, one filled action (New ticket), and the thread in a modal.
import { useForm } from '@mantine/form';
import { useQueryClient } from '@tanstack/react-query';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createTicket from '@/api/me/tickets/createTicket.ts';
import getTicket from '@/api/me/tickets/getTicket.ts';
import getTickets from '@/api/me/tickets/getTickets.ts';
import replyTicket from '@/api/me/tickets/replyTicket.ts';
import setTicketStatus from '@/api/me/tickets/setTicketStatus.ts';
import Button from '@/elements/buttons/Button.tsx';
import UnstyledButton from '@/elements/buttons/UnstyledButton.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Select from '@/elements/input/Select.tsx';
import ServerSelect from '@/elements/input/ServerSelect.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  ticketCategory,
  ticketCreateSchema,
  ticketSchema,
  ticketStatus,
  ticketSummarySchema,
} from '@/lib/schemas/user/tickets.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Ticket = z.infer<typeof ticketSummarySchema>;
type Thread = z.infer<typeof ticketSchema>;
type TicketStatus = z.infer<typeof ticketStatus>;
type TicketCategory = z.infer<typeof ticketCategory>;

const base = 'pages.account.tickets';

const statusColor: Record<TicketStatus, string> = {
  open: 'blue',
  waiting: 'yellow',
  resolved: 'green',
  closed: 'gray',
};

function CreateModal({ opened, onClose }: { opened: boolean; onClose: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [loading, setLoading] = useState(false);
  const [serverUuid, setServerUuid] = useState<string | null>(null);

  const form = useForm<z.infer<typeof ticketCreateSchema>>({
    initialValues: { subject: '', category: 'general', serverUuid: null, body: '' },
    validate: zod4Resolver(ticketCreateSchema),
  });

  const submit = (values: z.infer<typeof ticketCreateSchema>) => {
    setLoading(true);
    createTicket({ ...values, serverUuid })
      .then(() => {
        addToast(t(`${base}.toast.created`, {}), 'success');
        form.reset();
        setServerUuid(null);
        onClose();
        queryClient.invalidateQueries({ queryKey: queryKeys.user.tickets() });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal opened={opened} onClose={onClose} title={t(`${base}.create.title`, {})} size='lg'>
      <form onSubmit={form.onSubmit(submit)}>
        <Stack gap='md'>
          <TextInput
            withAsterisk
            label={t(`${base}.create.subject`, {})}
            maxLength={200}
            {...form.getInputProps('subject')}
          />
          <Select
            withAsterisk
            label={t(`${base}.create.category`, {})}
            data={(['general', 'billing', 'technical', 'abuse', 'other'] as const).map((category) => ({
              value: category,
              label: t(`${base}.category.${category}`, {}),
            }))}
            {...form.getInputProps('category')}
          />
          <ServerSelect
            label={t(`${base}.create.server`, {})}
            description={t(`${base}.create.serverHint`, {})}
            value={serverUuid}
            onChange={(uuid) => setServerUuid(uuid)}
            clearable
          />
          <TextArea
            withAsterisk
            autosize
            minRows={4}
            maxRows={12}
            label={t(`${base}.create.message`, {})}
            description={t(`${base}.create.messageHint`, {})}
            {...form.getInputProps('body')}
          />
        </Stack>

        <ModalFooter>
          <Button type='submit' loading={loading}>
            {t(`${base}.create.button`, {})}
          </Button>
          <Button variant='default' onClick={onClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}

function ThreadModal({ uuid, onClose }: { uuid: string | null; onClose: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [thread, setThread] = useState<Thread | null>(null);
  const [loading, setLoading] = useState(false);
  const [reply, setReply] = useState('');
  const [sending, setSending] = useState(false);

  const load = () => {
    if (!uuid) return;
    setLoading(true);
    getTicket(uuid)
      .then(setThread)
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    if (!uuid) {
      setThread(null);
      setReply('');
      return;
    }
    load();
  }, [uuid]);

  const send = () => {
    if (!uuid || !reply.trim()) return;
    setSending(true);
    replyTicket(uuid, reply.trim())
      .then((next) => {
        setThread(next);
        setReply('');
        queryClient.invalidateQueries({ queryKey: queryKeys.user.tickets() });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSending(false));
  };

  const changeStatus = (status: TicketStatus) => {
    if (!uuid) return;
    setTicketStatus(uuid, status)
      .then((next) => {
        setThread(next);
        queryClient.invalidateQueries({ queryKey: queryKeys.user.tickets() });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'));
  };

  return (
    <Modal
      opened={uuid !== null}
      onClose={() => {
        setThread(null);
        setReply('');
        onClose();
      }}
      title={thread?.ticket.subject ?? t(`${base}.thread.title`, {})}
      size='lg'
    >
      {loading && !thread ? (
        <Text size='sm' c='dimmed'>
          {t('common.loading', {})}
        </Text>
      ) : thread ? (
        <Stack gap='md'>
          <div className='flex flex-wrap items-center gap-2'>
            <Badge color={statusColor[thread.ticket.status]}>{t(`${base}.status.${thread.ticket.status}`, {})}</Badge>
            <Badge color='gray' variant='light'>
              {t(`${base}.category.${thread.ticket.category}`, {})}
            </Badge>
            {thread.ticket.server && (
              <Text size='sm' c='dimmed'>
                {thread.ticket.server.name}
              </Text>
            )}
            <Text size='sm' c='dimmed'>
              {t(`${base}.thread.updated`, {})} <FormattedTimestamp timestamp={thread.ticket.updated} />
            </Text>
          </div>

          <div className='theme-eco-list'>
            {thread.messages.map((message) => (
              <div key={message.uuid} className='theme-eco-tx'>
                <div className='flex min-w-0 flex-col'>
                  <span className='theme-eco-small'>
                    {message.authorType === 'neo'
                      ? t(`${base}.thread.neo`, {})
                      : message.authorType === 'system'
                        ? t(`${base}.thread.system`, {})
                        : message.authorType === 'admin'
                          ? t(`${base}.thread.staff`, {})
                          : (message.author?.username ?? t(`${base}.thread.you`, {}))}
                    {' · '}
                    <FormattedTimestamp timestamp={message.created} />
                  </span>
                  <p className='whitespace-pre-wrap theme-eco-text'>{message.body}</p>
                </div>
              </div>
            ))}
          </div>

          {thread.ticket.status !== 'closed' ? (
            <Stack gap='xs'>
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
                <Button variant='default' onClick={() => changeStatus('closed')}>
                  {t(`${base}.thread.close`, {})}
                </Button>
              </div>
            </Stack>
          ) : (
            <div className='flex flex-wrap gap-2'>
              <Button variant='default' onClick={() => changeStatus('open')}>
                {t(`${base}.thread.reopen`, {})}
              </Button>
            </div>
          )}
        </Stack>
      ) : null}
    </Modal>
  );
}

export default function DashboardTickets() {
  const { t } = useTranslations();
  const [creating, setCreating] = useState(false);
  const [opening, setOpening] = useState<string | null>(null);

  const resource = useResource({
    queryKey: queryKeys.user.tickets(),
    queryFn: getTickets,
  });

  return (
    <AccountContentContainer
      title={t(`${base}.title`, {})}
      subtitle={t(`${base}.subtitle`, {})}
      contentRight={<Button onClick={() => setCreating(true)}>{t(`${base}.create.title`, {})}</Button>}
    >
      <CreateModal opened={creating} onClose={() => setCreating(false)} />
      <ThreadModal uuid={opening} onClose={() => setOpening(null)} />

      <ResourceView resource={resource}>
        {(data) => (
          <Table
            columns={[
              t(`${base}.table.subject`, {}),
              t(`${base}.table.status`, {}),
              t(`${base}.table.category`, {}),
              t(`${base}.table.server`, {}),
              t(`${base}.table.updated`, {}),
              '',
            ]}
            pagination={{
              total: data.tickets.length,
              perPage: Math.max(data.tickets.length, 1),
              page: 1,
              data: data.tickets,
            }}
            empty={<EmptyState flush title={t(`${base}.empty`, {})} description={t(`${base}.emptyHint`, {})} />}
          >
            {data.tickets.map((ticket) => (
              <TableRow key={ticket.uuid}>
                <TableData>
                  <UnstyledButton className='theme-link-action' onClick={() => setOpening(ticket.uuid)}>
                    {ticket.subject}
                  </UnstyledButton>
                </TableData>
                <TableData>
                  <Badge color={statusColor[ticket.status]}>{t(`${base}.status.${ticket.status}`, {})}</Badge>
                </TableData>
                <TableData>{t(`${base}.category.${ticket.category}`, {})}</TableData>
                <TableData>{ticket.server?.name ?? t(`${base}.noServer`, {})}</TableData>
                <TableData>
                  <FormattedTimestamp timestamp={ticket.updated} />
                </TableData>
                <TableData className='text-end'>
                  <UnstyledButton className='theme-link-action' onClick={() => setOpening(ticket.uuid)}>
                    {t('common.button.view', {})}
                  </UnstyledButton>
                </TableData>
              </TableRow>
            ))}
          </Table>
        )}
      </ResourceView>
    </AccountContentContainer>
  );
}
