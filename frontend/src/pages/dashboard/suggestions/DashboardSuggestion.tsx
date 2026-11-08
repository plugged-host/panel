// plugged.host: one suggestion as a full page: its description, concept art and discussion
// (docs/features/suggestions.md).
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useParams } from 'react-router';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import SuggestionThread from '@/elements/suggestions/SuggestionThread.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import type { Suggestion } from '@/lib/schemas/user/suggestions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const base = 'pages.account.suggestions';

export default function DashboardSuggestion() {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const { id } = useParams<'id'>();
  const [suggestion, setSuggestion] = useState<Suggestion | null>(null);

  return (
    <AccountContentContainer title={suggestion?.title ?? t(`${base}.thread.title`, {})}>
      {id && (
        <SuggestionThread
          uuid={id}
          onSuggestion={setSuggestion}
          onChanged={() => queryClient.invalidateQueries({ queryKey: queryKeys.user.suggestions() })}
        />
      )}
    </AccountContentContainer>
  );
}
