// plugged.host: Admin › Suggestions (docs/features/suggestions.md): move an idea through its
// statuses, write the changelog entry, merge duplicates and delete.
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import deleteSuggestion from '@/api/admin/suggestions/deleteSuggestion.ts';
import getSuggestions from '@/api/admin/suggestions/getSuggestions.ts';
import mergeSuggestion from '@/api/admin/suggestions/mergeSuggestion.ts';
import updateSuggestion from '@/api/admin/suggestions/updateSuggestion.ts';
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
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  type SuggestionStatus,
  suggestionSchema,
  suggestionServerType,
  suggestionStatus,
} from '@/lib/schemas/user/suggestions.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Suggestion = z.infer<typeof suggestionSchema>;
type EditableStatus = Exclude<SuggestionStatus, 'merged'>;

const base = 'pages.admin.suggestions';
const EDITABLE: EditableStatus[] = ['under_review', 'planned', 'in_progress', 'shipped', 'declined'];

const statusColor: Record<SuggestionStatus, string> = {
  under_review: 'gray',
  planned: 'blue',
  in_progress: 'yellow',
  shipped: 'green',
  declined: 'red',
  merged: 'gray',
};

function EditModal({ suggestion, onClose }: { suggestion: Suggestion | null; onClose: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<EditableStatus>('under_review');
  const [changelog, setChangelog] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!suggestion) return;
    setStatus(
      (EDITABLE.includes(suggestion.status as EditableStatus) ? suggestion.status : 'under_review') as EditableStatus,
    );
    setChangelog(suggestion.changelog ?? '');
  }, [suggestion?.uuid]);

  const save = () => {
    if (!suggestion) return;
    setLoading(true);
    updateSuggestion(suggestion.uuid, { status, changelog: changelog.trim() || null })
      .then(() => {
        addToast(t(`${base}.toast.updated`, {}), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.suggestions.all() });
        onClose();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal
      opened={suggestion !== null}
      onClose={onClose}
      title={suggestion?.title ?? t(`${base}.edit.title`, {})}
      size='lg'
    >
      <Stack gap='md'>
        <Select
          label={t(`${base}.edit.status`, {})}
          data={EDITABLE.map((value) => ({
            value,
            label: t(`pages.account.suggestions.status.${value}`, {}),
          }))}
          value={status}
          onChange={(value) => value && setStatus(value as EditableStatus)}
          allowDeselect={false}
        />
        <TextArea
          autosize
          minRows={3}
          maxRows={10}
          label={t(`${base}.edit.changelog`, {})}
          description={t(`${base}.edit.changelogHint`, {})}
          value={changelog}
          onChange={(event) => setChangelog(event.currentTarget.value)}
        />
      </Stack>

      <ModalFooter>
        <Button loading={loading} onClick={save}>
          {t('common.button.save', {})}
        </Button>
        <Button variant='default' onClick={onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

function MergeModal({
  suggestion,
  suggestions,
  onClose,
}: {
  suggestion: Suggestion | null;
  suggestions: Suggestion[];
  onClose: () => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [target, setTarget] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => setTarget(null), [suggestion?.uuid]);

  const merge = () => {
    if (!suggestion || !target) return;
    setLoading(true);
    mergeSuggestion(suggestion.uuid, target)
      .then(() => {
        addToast(t(`${base}.toast.merged`, {}), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.suggestions.all() });
        onClose();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal opened={suggestion !== null} onClose={onClose} title={t(`${base}.merge.title`, {})} size='lg'>
      <Stack gap='md'>
        <Text size='sm'>{t(`${base}.merge.body`, { title: suggestion?.title ?? '' })}</Text>
        <Select
          label={t(`${base}.merge.target`, {})}
          data={suggestions
            .filter((item) => item.uuid !== suggestion?.uuid && item.status !== 'merged')
            .map((item) => ({ value: item.uuid, label: item.title }))}
          value={target}
          onChange={setTarget}
          searchable
        />
      </Stack>

      <ModalFooter>
        <Button loading={loading} disabled={!target} onClick={merge}>
          {t(`${base}.merge.button`, {})}
        </Button>
        <Button variant='default' onClick={onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

export default function AdminSuggestions() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canManage = useAdminCan('suggestions.manage');
  const [status, setStatus] = useState<SuggestionStatus | null>(null);
  const [serverType, setServerType] = useState<string | null>(null);
  const [editing, setEditing] = useState<Suggestion | null>(null);
  const [merging, setMerging] = useState<Suggestion | null>(null);
  const [deleting, setDeleting] = useState<Suggestion | null>(null);

  const { data, loading, error, search, setSearch, setPage, setPerPage } = useSearchablePaginatedTable({
    queryKey: [...queryKeys.admin.suggestions.all(), { status, serverType }],
    fetcher: (page, search, perPage) => getSuggestions(page, search, perPage, status, serverType),
    deps: [status, serverType],
  });

  const remove = async () => {
    if (!deleting) return;
    try {
      await deleteSuggestion(deleting.uuid);
      addToast(t(`${base}.toast.deleted`, {}), 'success');
      setDeleting(null);
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.suggestions.all() });
    } catch (error) {
      addToast(httpErrorToHuman(error), 'error');
    }
  };

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
            onChange={(value) => setStatus(value as SuggestionStatus | null)}
            data={(['under_review', 'planned', 'in_progress', 'shipped', 'declined', 'merged'] as const).map(
              (value) => ({ value, label: t(`pages.account.suggestions.status.${value}`, {}) }),
            )}
            clearable
            w={170}
          />
          <Select
            placeholder={t(`${base}.filter.serverType`, {})}
            value={serverType}
            onChange={setServerType}
            data={(['minecraft', 'fivem', 'rust', 'teamspeak', 'other'] as const).map((value) => ({
              value,
              label: t(`pages.account.suggestions.serverType.${value}`, {}),
            }))}
            clearable
            w={170}
          />
        </div>
      }
    >
      <EditModal suggestion={editing} onClose={() => setEditing(null)} />
      <MergeModal suggestion={merging} suggestions={data?.data ?? []} onClose={() => setMerging(null)} />
      <ConfirmationModal
        opened={deleting !== null}
        onClose={() => setDeleting(null)}
        title={t(`${base}.delete.title`, {})}
        confirm={t('common.button.delete', {})}
        onConfirmed={remove}
      >
        <Text size='sm'>{t(`${base}.delete.body`, { title: deleting?.title ?? '' })}</Text>
      </ConfirmationModal>

      <Table
        columns={[
          t(`${base}.table.title`, {}),
          t(`${base}.table.author`, {}),
          t(`${base}.table.type`, {}),
          t(`${base}.table.votes`, {}),
          t(`${base}.table.status`, {}),
          t(`${base}.table.created`, {}),
          '',
        ]}
        loading={loading}
        error={error}
        pagination={data}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {data?.data.map((suggestion) => (
          <TableRow key={suggestion.uuid}>
            <TableData>
              <div className='flex min-w-0 flex-col'>
                <span className='font-medium'>{suggestion.title}</span>
                <span className='theme-eco-small'>{suggestion.body}</span>
              </div>
            </TableData>
            <TableData>{suggestion.author?.username}</TableData>
            <TableData>{t(`pages.account.suggestions.serverType.${suggestion.serverType}`, {})}</TableData>
            <TableData>{suggestion.votes}</TableData>
            <TableData>
              <Badge color={statusColor[suggestion.status]}>
                {t(`pages.account.suggestions.status.${suggestion.status}`, {})}
              </Badge>
            </TableData>
            <TableData>
              <FormattedTimestamp timestamp={suggestion.created} />
            </TableData>
            <TableData className='text-end'>
              {canManage && suggestion.status !== 'merged' && (
                <div className='theme-link-actions'>
                  <UnstyledButton className='theme-link-action' onClick={() => setEditing(suggestion)}>
                    {t('common.button.edit', {})}
                  </UnstyledButton>
                  <UnstyledButton className='theme-link-action' onClick={() => setMerging(suggestion)}>
                    {t(`${base}.merge.button`, {})}
                  </UnstyledButton>
                  <UnstyledButton className='theme-link-action' data-danger onClick={() => setDeleting(suggestion)}>
                    {t('common.button.delete', {})}
                  </UnstyledButton>
                </div>
              )}
            </TableData>
          </TableRow>
        ))}
      </Table>
    </AdminContentContainer>
  );
}
