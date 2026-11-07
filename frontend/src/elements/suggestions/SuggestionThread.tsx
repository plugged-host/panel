// plugged.host: one idea's page: the full description, its concept art and the discussion
// (docs/features/suggestions.md). Staff replies use the admin API and are marked Staff.
import { faPaperPlane } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import commentAdminSuggestion from '@/api/admin/suggestions/commentAdminSuggestion.ts';
import getAdminSuggestion from '@/api/admin/suggestions/getAdminSuggestion.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import commentSuggestion from '@/api/me/suggestions/commentSuggestion.ts';
import getSuggestion from '@/api/me/suggestions/getSuggestion.ts';
import toggleSuggestionVote from '@/api/me/suggestions/toggleSuggestionVote.ts';
import uploadSuggestionAttachments from '@/api/me/suggestions/uploadSuggestionAttachments.ts';
import Button from '@/elements/buttons/Button.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import FileInput from '@/elements/input/FileInput.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import NeoMarkdown from '@/elements/neo/NeoMarkdown.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { type Suggestion, suggestionThreadSchema } from '@/lib/schemas/user/suggestions.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Thread = z.infer<typeof suggestionThreadSchema>;

const base = 'pages.account.suggestions';
const statusColor: Record<string, string> = {
  under_review: 'gray',
  planned: 'blue',
  in_progress: 'yellow',
  shipped: 'green',
  declined: 'red',
  merged: 'gray',
};

export default function SuggestionThread({
  uuid,
  admin = false,
  onSuggestion,
  onChanged,
}: {
  uuid: string;
  admin?: boolean;
  onSuggestion?: (suggestion: Suggestion | null) => void;
  onChanged?: () => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const { user } = useAuth();
  const _queryClient = useQueryClient();
  const [thread, setThread] = useState<Thread | null>(null);
  const [loading, setLoading] = useState(true);
  const [comment, setComment] = useState('');
  const [sending, setSending] = useState(false);
  const [files, setFiles] = useState<File[]>([]);
  const [uploading, setUploading] = useState(false);
  const [voting, setVoting] = useState(false);

  useEffect(() => {
    let live = true;
    setLoading(true);
    (admin ? getAdminSuggestion(uuid) : getSuggestion(uuid))
      .then((next) => {
        if (!live) return;
        setThread(next);
        onSuggestion?.(next.suggestion);
      })
      .catch((error) => {
        if (live) addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => {
        if (live) setLoading(false);
      });
    return () => {
      live = false;
    };
  }, [uuid, admin]);

  const vote = () => {
    setVoting(true);
    toggleSuggestionVote(uuid)
      .then((next) => {
        setThread((current) => (current ? { ...current, suggestion: next } : current));
        onSuggestion?.(next);
        onChanged?.();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setVoting(false));
  };

  const send = () => {
    if (!comment.trim()) return;
    setSending(true);
    (admin ? commentAdminSuggestion : commentSuggestion)(uuid, comment.trim())
      .then((next) => {
        setThread(next);
        onSuggestion?.(next.suggestion);
        setComment('');
        onChanged?.();
        addToast(t(`${base}.thread.posted`, {}), 'success');
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSending(false));
  };

  const upload = () => {
    if (files.length === 0) return;
    setUploading(true);
    uploadSuggestionAttachments(uuid, files)
      .then((attachments) => {
        setThread((current) =>
          current ? { ...current, attachments: [...current.attachments, ...attachments] } : current,
        );
        setFiles([]);
        onChanged?.();
        addToast(t(`${base}.thread.imagesAdded`, {}), 'success');
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setUploading(false));
  };

  if (loading && !thread) {
    return (
      <Text size='sm' c='dimmed'>
        {t('common.loading', {})}
      </Text>
    );
  }
  if (!thread) return null;

  const suggestion = thread.suggestion;
  const canAttach = !admin && (user?.admin || user?.uuid === suggestion.author?.uuid);

  return (
    <Stack gap='lg'>
      <div className='flex flex-wrap items-center gap-2'>
        <Badge color={statusColor[suggestion.status]}>{t(`${base}.status.${suggestion.status}`, {})}</Badge>
        <Badge color='gray' variant='light'>
          {t(`${base}.serverType.${suggestion.serverType}`, {})}
        </Badge>
        <Text size='sm' c='dimmed'>
          {suggestion.author?.username ?? t(`${base}.anonymous`, {})}
          ·
          <FormattedTimestamp timestamp={suggestion.created} />
        </Text>
        {admin ? (
          <Text size='sm' c='dimmed' className='ml-auto'>
            ▲ {suggestion.votes}
          </Text>
        ) : (
          <Button
            className='ml-auto'
            variant={suggestion.voted ? 'light' : 'default'}
            size='xs'
            loading={voting}
            onClick={vote}
            aria-label={t(`${base}.vote`, {})}
          >
            ▲ {suggestion.votes}
          </Button>
        )}
      </div>

      <section className='theme-eco-card'>
        <NeoMarkdown text={suggestion.body} />
        {suggestion.changelog && suggestion.status === 'shipped' && (
          <p className='theme-eco-small'>{t(`${base}.changelog`, { entry: suggestion.changelog })}</p>
        )}
        {suggestion.mergedInto && (
          <p className='theme-eco-small'>{t(`${base}.mergedInto`, { title: suggestion.mergedInto.title })}</p>
        )}
      </section>

      {(thread.attachments.length > 0 || canAttach) && (
        <section className='theme-eco-card'>
          <h3 className='theme-eco-heading'>{t('pages.account.tickets.thread.attachments', {})}</h3>
          {thread.attachments.length > 0 && (
            <div className='flex flex-wrap gap-3'>
              {thread.attachments.map((attachment) =>
                attachment.mime.startsWith('image/') ? (
                  <a key={attachment.uuid} href={attachment.url} target='_blank' rel='noopener noreferrer'>
                    <img
                      src={attachment.url}
                      alt={attachment.name}
                      className='max-h-64 rounded-lg border border-(--theme-hairline) object-contain'
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
          )}
          {canAttach && (
            <div className='flex flex-col gap-2'>
              <FileInput
                label={t(`${base}.thread.addImages`, {})}
                description={t(`${base}.thread.attachmentsHint`, {})}
                multiple
                clearable
                accept='image/*,application/pdf,text/plain,application/json,application/zip,application/gzip'
                value={files}
                onChange={(value) => setFiles(Array.isArray(value) ? value : value ? [value] : [])}
              />
              <div>
                <Button size='xs' loading={uploading} disabled={files.length === 0} onClick={upload}>
                  {t(`${base}.thread.addImages`, {})}
                </Button>
              </div>
            </div>
          )}
        </section>
      )}

      <section className='flex flex-col gap-3'>
        <h3 className='theme-eco-heading'>{t(`${base}.thread.comments`, {})}</h3>
        {thread.messages.length === 0 ? (
          <Text size='sm' c='dimmed'>
            {t(`${base}.thread.noComments`, {})}
          </Text>
        ) : (
          <div className='theme-eco-list'>
            {thread.messages.map((message) => (
              <div key={message.uuid} className='theme-eco-tx'>
                <div className='flex min-w-0 flex-col gap-1'>
                  <span className='theme-eco-small'>
                    {message.authorType === 'admin'
                      ? t(`${base}.thread.staff`, {})
                      : (message.author?.username ?? t(`${base}.thread.you`, {}))}
                    ·
                    <FormattedTimestamp timestamp={message.created} />
                  </span>
                  <p className='whitespace-pre-wrap theme-eco-text'>{message.body}</p>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className='theme-eco-card'>
        <h3 className='theme-eco-heading'>
          {admin ? t('pages.admin.tickets.thread.reply', {}) : t(`${base}.thread.addComment`, {})}
        </h3>
        <TextArea
          autosize
          minRows={3}
          maxRows={8}
          value={comment}
          onChange={(event) => setComment(event.currentTarget.value)}
          placeholder={t(`${base}.thread.addComment`, {})}
        />
        <div className='flex flex-wrap items-center gap-2'>
          <Button
            leftSection={<FontAwesomeIcon icon={faPaperPlane} />}
            loading={sending}
            disabled={!comment.trim()}
            onClick={send}
          >
            {admin ? t('pages.admin.tickets.thread.send', {}) : t(`${base}.thread.postComment`, {})}
          </Button>
        </div>
      </section>
    </Stack>
  );
}
