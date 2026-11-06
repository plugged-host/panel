// plugged.host: the suggestions board (shared/src/suggestions.rs, docs/features/suggestions.md)
import { z } from 'zod';

export const suggestionStatus = z.enum([
  'under_review',
  'planned',
  'in_progress',
  'shipped',
  'declined',
  'merged',
]);
export const suggestionServerType = z.enum(['minecraft', 'fivem', 'rust', 'teamspeak', 'other']);

export const suggestionUserSchema = z.object({
  uuid: z.string(),
  username: z.string(),
});

export const suggestionRefSchema = z.object({
  uuid: z.string(),
  title: z.string(),
});

export const suggestionSchema = z.object({
  uuid: z.string(),
  title: z.string(),
  body: z.string(),
  serverType: suggestionServerType,
  status: suggestionStatus,
  changelog: z.string().nullable(),
  votes: z.number(),
  voted: z.boolean(),
  author: suggestionUserSchema.nullable(),
  mergedInto: suggestionRefSchema.nullable(),
  created: z.coerce.date(),
  updated: z.coerce.date(),
});

export const suggestionsSchema = z.object({
  suggestions: z.array(suggestionSchema),
});

export const suggestionCreateSchema = z.object({
  title: z.string().min(3).max(120),
  body: z.string().min(10).max(500),
  serverType: suggestionServerType,
});

export type SuggestionStatus = z.infer<typeof suggestionStatus>;
export type SuggestionServerType = z.infer<typeof suggestionServerType>;
export type Suggestion = z.infer<typeof suggestionSchema>;
