// plugged.host: Account › Suggestions, the ideas board (docs/features/suggestions.md). Ideas by
// votes and then age, one vote per account, one filled action (New idea).

import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useForm } from '@mantine/form';
import { useQueryClient } from '@tanstack/react-query';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createSuggestion from '@/api/me/suggestions/createSuggestion.ts';
import getSuggestions from '@/api/me/suggestions/getSuggestions.ts';
import toggleSuggestionVote from '@/api/me/suggestions/toggleSuggestionVote.ts';
import uploadSuggestionAttachments from '@/api/me/suggestions/uploadSuggestionAttachments.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import FileInput from '@/elements/input/FileInput.tsx';
import Select from '@/elements/input/Select.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  suggestionCreateSchema,
  suggestionSchema,
  suggestionServerType,
  suggestionStatus,
} from '@/lib/schemas/user/suggestions.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Suggestion = z.infer<typeof suggestionSchema>;
type SuggestionStatus = z.infer<typeof suggestionStatus>;
type SuggestionServerType = z.infer<typeof suggestionServerType>;

const base = 'pages.account.suggestions';

const statusColor: Record<SuggestionStatus, string> = {
  under_review: 'gray',
  planned: 'blue',
  in_progress: 'yellow',
  shipped: 'green',
  declined: 'red',
  merged: 'gray',
};

const SERVER_TYPES: SuggestionServerType[] = ['minecraft', 'fivem', 'rust', 'teamspeak', 'other'];

function CreateModal({ opened, onClose }: { opened: boolean; onClose: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [loading, setLoading] = useState(false);
  const [files, setFiles] = useState<File[]>([]);

  const form = useForm<z.infer<typeof suggestionCreateSchema>>({
    initialValues: { title: '', body: '', serverType: 'other' },
    validate: zod4Resolver(suggestionCreateSchema),
  });

  const submit = (values: z.infer<typeof suggestionCreateSchema>) => {
    setLoading(true);
    createSuggestion(values)
      .then(async (created) => {
        if (files.length > 0) {
          await uploadSuggestionAttachments(created.uuid, files);
        }
        addToast(t(`${base}.toast.created`, {}), 'success');
        form.reset();
        setFiles([]);
        onClose();
        queryClient.invalidateQueries({ queryKey: queryKeys.user.suggestions() });
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
            label={t(`${base}.create.titleField`, {})}
            maxLength={120}
            {...form.getInputProps('title')}
          />
          <TextArea
            withAsterisk
            autosize
            minRows={5}
            maxRows={14}
            label={t(`${base}.create.description`, {})}
            description={t(`${base}.create.descriptionHint`, {})}
            {...form.getInputProps('body')}
          />
          <FileInput
            label={t(`${base}.create.attachments`, {})}
            description={t(`${base}.create.attachmentsHint`, {})}
            multiple
            clearable
            accept='image/*,application/pdf,text/plain,application/json,application/zip,application/gzip'
            value={files}
            onChange={(value) => setFiles(Array.isArray(value) ? value : value ? [value] : [])}
          />
          <Select
            withAsterisk
            label={t(`${base}.create.serverType`, {})}
            data={SERVER_TYPES.map((type) => ({ value: type, label: t(`${base}.serverType.${type}`, {}) }))}
            {...form.getInputProps('serverType')}
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

function Idea({ suggestion }: { suggestion: Suggestion }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [voting, setVoting] = useState(false);

  const vote = () => {
    setVoting(true);
    toggleSuggestionVote(suggestion.uuid)
      .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.user.suggestions() }))
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setVoting(false));
  };

  return (
    <div
      className='theme-eco-row cursor-pointer'
      role='link'
      tabIndex={0}
      onClick={() => navigate(`/account/suggestions/${suggestion.uuid}`)}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          navigate(`/account/suggestions/${suggestion.uuid}`);
        }
      }}
    >
      <div className='flex min-w-0 flex-1 flex-col gap-4'>
        {suggestion.previewUrl && (
          <img
            src={suggestion.previewUrl}
            alt=''
            className='max-h-64 w-full rounded-lg border border-(--theme-hairline) object-cover'
          />
        )}
        <div className='flex min-w-0 flex-1 items-start gap-4'>
          <Button
            variant={suggestion.voted ? 'light' : 'default'}
            size='xs'
            loading={voting}
            onClick={(event) => {
              event.stopPropagation();
              vote();
            }}
            aria-label={t(`${base}.vote`, {})}
          >
            ▲ {suggestion.votes}
          </Button>
          <div className='flex min-w-0 flex-1 flex-col gap-2'>
            <div className='flex flex-wrap items-center gap-2'>
              <h3 className='theme-eco-heading'>{suggestion.title}</h3>
              <Badge color={statusColor[suggestion.status]}>{t(`${base}.status.${suggestion.status}`, {})}</Badge>
              <Badge color='gray' variant='light'>
                {t(`${base}.serverType.${suggestion.serverType}`, {})}
              </Badge>
            </div>
            <p
              className='theme-eco-text'
              style={{
                display: '-webkit-box',
                WebkitLineClamp: 4,
                WebkitBoxOrient: 'vertical',
                overflow: 'hidden',
              }}
            >
              {suggestion.body}
            </p>
            {suggestion.changelog && suggestion.status === 'shipped' && (
              <p className='theme-eco-small'>{t(`${base}.changelog`, { entry: suggestion.changelog })}</p>
            )}
            {suggestion.mergedInto && (
              <p className='theme-eco-small'>{t(`${base}.mergedInto`, { title: suggestion.mergedInto.title })}</p>
            )}
            <div className='flex flex-wrap items-center gap-3'>
              <span className='theme-eco-small inline-flex flex-wrap items-baseline gap-3'>
                <span>{suggestion.author?.username ?? t(`${base}.anonymous`, {})}</span>
                <FormattedTimestamp timestamp={suggestion.created} />
              </span>
              <span className='theme-eco-small'>
                {suggestion.comments} {t(`${base}.thread.comments`, {})} · {suggestion.attachments}{' '}
                {t('pages.account.tickets.thread.attachments', {})}
              </span>
              <span className='theme-eco-link ml-auto'>{t(`${base}.viewDiscussion`, {})} →</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default function DashboardSuggestions() {
  const { t } = useTranslations();
  const [creating, setCreating] = useState(false);

  const resource = useResource({
    queryKey: queryKeys.user.suggestions(),
    queryFn: getSuggestions,
  });

  return (
    <AccountContentContainer
      title={t(`${base}.title`, {})}
      subtitle={t(`${base}.subtitle`, {})}
      contentRight={
        <Button leftSection={<FontAwesomeIcon icon={faPlus} />} onClick={() => setCreating(true)}>
          {t(`${base}.create.title`, {})}
        </Button>
      }
    >
      <CreateModal opened={creating} onClose={() => setCreating(false)} />

      <ResourceView resource={resource}>
        {(data) =>
          data.suggestions.length === 0 ? (
            <EmptyState flush title={t(`${base}.empty`, {})} description={t(`${base}.emptyHint`, {})} />
          ) : (
            <div className='theme-eco-list'>
              {data.suggestions.map((suggestion) => (
                <Idea key={suggestion.uuid} suggestion={suggestion} />
              ))}
            </div>
          )
        }
      </ResourceView>
    </AccountContentContainer>
  );
}
