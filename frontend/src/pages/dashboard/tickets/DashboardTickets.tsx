// plugged.host: Account › Support, the account's tickets (docs/features/tickets.md). A full-width
// list, one filled action (New ticket), and the thread in a modal.
import { useForm } from '@mantine/form';
import { useQueryClient } from '@tanstack/react-query';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createTicket from '@/api/me/tickets/createTicket.ts';
import deleteTicket from '@/api/me/tickets/deleteTicket.ts';
import getTicket from '@/api/me/tickets/getTicket.ts';
import getTickets from '@/api/me/tickets/getTickets.ts';
import replyTicket from '@/api/me/tickets/replyTicket.ts';
import setTicketStatus from '@/api/me/tickets/setTicketStatus.ts';
import uploadTicketAttachments from '@/api/me/tickets/uploadTicketAttachments.ts';
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
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { ticketCreateSchema, ticketSchema, ticketStatus } from '@/lib/schemas/user/tickets.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Thread = z.infer<typeof ticketSchema>;
type TicketStatus = z.infer<typeof ticketStatus>;

const base = 'pages.account.tickets';

const statusColor: Record<TicketStatus, string> = {
  open: 'blue',
  waiting: 'yellow',
  resolved: 'green',
  closed: 'gray',
};

function CreateModal({
  opened,
  onClose,
  onCreated,
}: {
  opened: boolean;
  onClose: () => void;
  onCreated: (uuid: string) => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [loading, setLoading] = useState(false);
  const [serverUuid, setServerUuid] = useState<string | null>(null);
  const [files, setFiles] = useState<File[]>([]);

  const form = useForm<z.infer<typeof ticketCreateSchema>>({
    initialValues: {
      subject: '',
      category: 'general',
      serverUuid: null,
      body: '',
      bugSteps: '',
      bugExpected: '',
      bugActual: '',
      bugEnvironment: '',
    },
    validate: zod4Resolver(ticketCreateSchema),
  });

  const submit = (values: z.infer<typeof ticketCreateSchema>) => {
    setLoading(true);
    createTicket({ ...values, serverUuid })
      .then(async (created) => {
        if (files.length > 0) {
          await uploadTicketAttachments(created.ticket.uuid, files);
        }
        addToast(t(`${base}.toast.created`, {}), 'success');
        form.reset();
        setServerUuid(null);
        setFiles([]);
        onClose();
        queryClient.invalidateQueries({ queryKey: queryKeys.user.tickets() });
        onCreated(created.ticket.uuid);
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
            data={(['general', 'billing', 'technical', 'bug', 'abuse', 'other'] as const).map((category) => ({
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
          {form.values.category === 'bug' && (
            <>
              <TextArea
                autosize
                minRows={3}
                label={t(`${base}.create.bugSteps`, {})}
                description={t(`${base}.create.bugStepsHint`, {})}
                {...form.getInputProps('bugSteps')}
              />
              <TextArea
                autosize
                minRows={2}
                label={t(`${base}.create.bugExpected`, {})}
                {...form.getInputProps('bugExpected')}
              />
              <TextArea
                autosize
                minRows={2}
                label={t(`${base}.create.bugActual`, {})}
                {...form.getInputProps('bugActual')}
              />
              <TextInput label={t(`${base}.create.bugEnvironment`, {})} {...form.getInputProps('bugEnvironment')} />
            </>
          )}
          <div className='field'>
            <label className='label' htmlFor='ticket-attachments'>
              {t(`${base}.create.attachments`, {})}
            </label>
            <input
              id='ticket-attachments'
              className='input'
              type='file'
              multiple
              accept='image/*,application/pdf,text/plain,application/json,application/zip,application/gzip'
              onChange={(event) => setFiles(Array.from(event.currentTarget.files ?? []))}
            />
            <p className='hint'>{t(`${base}.create.attachmentsHint`, {})}</p>
          </div>
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

export function ThreadModal({
  uuid,
  onClose,
  inline = false,
}: {
  uuid: string | null;
  onClose: () => void;
  inline?: boolean;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [thread, setThread] = useState<Thread | null>(null);
  const [loading, setLoading] = useState(false);
  const [reply, setReply] = useState('');
  const [sending, setSending] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const load = (silent = false) => {
    if (!uuid) return;
    if (!silent) setLoading(true);
    getTicket(uuid)
      .then(setThread)
      .catch((error) => {
        if (!silent) addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => {
        if (!silent) setLoading(false);
      });
  };

  useEffect(() => {
    if (!uuid) {
      setThread(null);
      setReply('');
      return;
    }
    load();
    const timer = window.setInterval(() => load(true), 2_000);
    return () => window.clearInterval(timer);
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

  const content = (
    <>
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

          {thread.ticket.bug && (
            <div className='theme-eco-list'>
              <div className='theme-eco-tx'>
                <div className='flex min-w-0 flex-col gap-1'>
                  <span className='theme-eco-small'>{t(`${base}.thread.bug`, {})}</span>
                  {thread.ticket.bug.steps && (
                    <p className='theme-eco-text'>
                      <strong>{t(`${base}.thread.bugSteps`, {})}:</strong> {thread.ticket.bug.steps}
                    </p>
                  )}
                  {thread.ticket.bug.expected && (
                    <p className='theme-eco-text'>
                      <strong>{t(`${base}.thread.bugExpected`, {})}:</strong> {thread.ticket.bug.expected}
                    </p>
                  )}
                  {thread.ticket.bug.actual && (
                    <p className='theme-eco-text'>
                      <strong>{t(`${base}.thread.bugActual`, {})}:</strong> {thread.ticket.bug.actual}
                    </p>
                  )}
                  {thread.ticket.bug.environment && (
                    <p className='theme-eco-text'>
                      <strong>{t(`${base}.thread.bugEnvironment`, {})}:</strong> {thread.ticket.bug.environment}
                    </p>
                  )}
                </div>
              </div>
            </div>
          )}

          {thread.attachments.length > 0 && (
            <div className='flex flex-col gap-2'>
              <span className='theme-eco-small'>{t(`${base}.thread.attachments`, {})}</span>
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
                      ? t(`${base}.thread.neo`, {})
                      : message.authorType === 'system'
                        ? t(`${base}.thread.system`, {})
                        : message.authorType === 'admin'
                          ? t(`${base}.thread.staff`, {})
                          : (message.author?.username ?? t(`${base}.thread.you`, {}))}
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

          <div className='flex flex-wrap gap-2'>
            <Button color='red' variant='outline' onClick={() => setConfirmDelete(true)}>
              {t('common.button.delete', {})}
            </Button>
          </div>

          <ConfirmationModal
            opened={confirmDelete}
            onClose={() => setConfirmDelete(false)}
            title={t(`${base}.thread.deleteTitle`, {})}
            confirm={t('common.button.delete', {})}
            confirmColor='red'
            onConfirmed={() => {
              if (!uuid) return;
              deleteTicket(uuid)
                .then(() => {
                  queryClient.invalidateQueries({ queryKey: queryKeys.user.tickets() });
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
      opened={uuid !== null}
      onClose={() => {
        setThread(null);
        setReply('');
        onClose();
      }}
      title={thread?.ticket.subject ?? t(`${base}.thread.title`, {})}
      size='lg'
    >
      {content}
    </Modal>
  );
}

export default function DashboardTickets() {
  const { t } = useTranslations();
  const [creating, setCreating] = useState(false);
  const navigate = useNavigate();

  const resource = useResource({
    queryKey: queryKeys.user.tickets(),
    queryFn: getTickets,
    refetchInterval: 10_000,
  });

  return (
    <AccountContentContainer
      title={t(`${base}.title`, {})}
      subtitle={t(`${base}.subtitle`, {})}
      contentRight={<Button onClick={() => setCreating(true)}>{t(`${base}.create.title`, {})}</Button>}
    >
      <CreateModal
        opened={creating}
        onClose={() => setCreating(false)}
        onCreated={(uuid) => navigate(`/account/support/${uuid}`)}
      />

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
                  <UnstyledButton
                    className='theme-link-action'
                    onClick={() => navigate(`/account/support/${ticket.uuid}`)}
                  >
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
                  <UnstyledButton
                    className='theme-link-action'
                    onClick={() => navigate(`/account/support/${ticket.uuid}`)}
                  >
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
