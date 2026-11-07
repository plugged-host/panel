// plugged.host: one suggestion as a full page: its description, concept art and discussion
// (docs/features/suggestions.md). The author's trash button sits at the top right.
import { faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router';
import { httpErrorToHuman } from '@/api/axios.ts';
import deleteSuggestion from '@/api/me/suggestions/deleteSuggestion.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import SuggestionThread from '@/elements/suggestions/SuggestionThread.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import type { Suggestion } from '@/lib/schemas/user/suggestions.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const base = 'pages.account.suggestions';

export default function DashboardSuggestion() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const { user } = useAuth();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { id } = useParams<'id'>();
  const [suggestion, setSuggestion] = useState<Suggestion | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const canDelete = !!suggestion && (user?.admin || user?.uuid === suggestion.author?.uuid);

  const remove = () => {
    if (!id) return;
    deleteSuggestion(id)
      .then(() => {
        queryClient.invalidateQueries({ queryKey: queryKeys.user.suggestions() });
        navigate('/account/suggestions');
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'));
  };

  return (
    <AccountContentContainer
      title={suggestion?.title ?? t(`${base}.thread.title`, {})}
      contentRight={
        canDelete ? (
          <ActionIcon
            color='red'
            variant='subtle'
            aria-label={t('common.button.delete', {})}
            title={t('common.button.delete', {})}
            onClick={() => setConfirmDelete(true)}
          >
            <FontAwesomeIcon icon={faTrash} />
          </ActionIcon>
        ) : undefined
      }
    >
      {id && (
        <SuggestionThread
          uuid={id}
          onSuggestion={setSuggestion}
          onChanged={() => queryClient.invalidateQueries({ queryKey: queryKeys.user.suggestions() })}
        />
      )}

      <ConfirmationModal
        opened={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        title={t(`${base}.deleteTitle`, {})}
        confirm={t('common.button.delete', {})}
        confirmColor='red'
        onConfirmed={remove}
      >
        {t(`${base}.deleteBody`, {})}
      </ConfirmationModal>
    </AccountContentContainer>
  );
}
