// plugged.host: Admin › Suggestions (shared/src/suggestions.rs, docs/features/suggestions.md)
import { z } from 'zod';
import { suggestionServerType, suggestionStatus } from '@/lib/schemas/user/suggestions.ts';

export const adminSuggestionStatusFilter = suggestionStatus;
export const adminSuggestionServerTypeFilter = suggestionServerType;

export const adminSuggestionUpdateSchema = z.object({
  status: suggestionStatus.exclude(['merged']),
  changelog: z.string().max(5000).nullable(),
});
