// plugged.host: Account › Suggestions, the ideas board (docs/features/suggestions.md). Ideas by
// votes and then age, one vote per account, one filled action (New idea).

import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useForm } from '@mantine/form';
import { useQueryClient } from '@tanstack/react-query';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createSuggestion from '@/api/me/suggestions/createSuggestion.ts';
import deleteSuggestion from '@/api/me/suggestions/deleteSuggestion.ts';
import getSuggestions from '@/api/me/suggestions/getSuggestions.ts';
import toggleSuggestionVote from '@/api/me/suggestions/toggleSuggestionVote.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Select from '@/elements/input/Select.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import NeoMarkdown from '@/elements/neo/NeoMarkdown.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  suggestionCreateSchema,
  suggestionSchema,
  suggestionServerType,
  suggestionStatus,
} from '@/lib/schemas/user/suggestions.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
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

  const form = useForm<z.infer<typeof suggestionCreateSchema>>({
    initialValues: { title: '', body: '', serverType: 'other' },
    validate: zod4Resolver(suggestionCreateSchema),
  });

  const submit = (values: z.infer<typeof suggestionCreateSchema>) => {
    setLoading(true);
    createSuggestion(values)
      .then(() => {
        addToast(t(`${base}.toast.created`, {}), 'success');
        form.reset();
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
            minRows={3}
            maxRows={8}
            label={t(`${base}.create.description`, {})}
            description={t(`${base}.create.descriptionHint`, {})}
            {...form.getInputProps('body')}
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
  const [voting, setVoting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const { user } = useAuth();

  const vote = () => {
    setVoting(true);
    toggleSuggestionVote(suggestion.uuid)
      .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.user.suggestions() }))
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setVoting(false));
  };

  return (
    <div className='theme-eco-card'>
      <div className='flex flex-col gap-3 sm:flex-row sm:items-start'>
        <Button
          variant={suggestion.voted ? 'filled' : 'default'}
          size='xs'
          loading={voting}
          onClick={vote}
          aria-label={t(`${base}.vote`, {})}
        >
          ▲ {suggestion.votes}
        </Button>
        <div className='flex min-w-0 flex-1 flex-col gap-1'>
          <div className='flex flex-wrap items-center gap-2'>
            <h3 className='theme-eco-heading'>{suggestion.title}</h3>
            <Badge color={statusColor[suggestion.status]}>{t(`${base}.status.${suggestion.status}`, {})}</Badge>
            <Badge color='gray' variant='light'>
              {t(`${base}.serverType.${suggestion.serverType}`, {})}
            </Badge>
          </div>
          <NeoMarkdown text={suggestion.body} />
          {suggestion.changelog && suggestion.status === 'shipped' && (
            <p className='theme-eco-small'>{t(`${base}.changelog`, { entry: suggestion.changelog })}</p>
          )}
          {suggestion.mergedInto && (
            <p className='theme-eco-small'>{t(`${base}.mergedInto`, { title: suggestion.mergedInto.title })}</p>
          )}
          <span className='theme-eco-small'>
            {suggestion.author?.username ?? t(`${base}.anonymous`, {})}
            {' · '}
            <FormattedTimestamp timestamp={suggestion.created} />
          </span>
        </div>
        {user?.uuid === suggestion.author?.uuid && (
          <Button color='red' variant='outline' size='xs' onClick={() => setConfirmDelete(true)}>
            {t('common.button.delete', {})}
          </Button>
        )}
      </div>

      <ConfirmationModal
        opened={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        title={t(`${base}.deleteTitle`, {})}
        confirm={t('common.button.delete', {})}
        confirmColor='red'
        onConfirmed={() => {
          deleteSuggestion(suggestion.uuid)
            .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.user.suggestions() }))
            .catch((error) => addToast(httpErrorToHuman(error), 'error'));
        }}
      >
        {t(`${base}.deleteBody`, {})}
      </ConfirmationModal>
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
