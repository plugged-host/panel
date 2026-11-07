// plugged.host: the suggestions board (shared/src/suggestions.rs, docs/features/suggestions.md)
import { z } from 'zod';

export const suggestionStatus = z.enum(['under_review', 'planned', 'in_progress', 'shipped', 'declined', 'merged']);
export const suggestionServerType = z.enum(['minecraft', 'fivem', 'rust', 'teamspeak', 'other']);

export const suggestionUserSchema = z.object({
  uuid: z.string(),
  username: z.string(),
  badge: z.string().nullable(),
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
  comments: z.number(),
  attachments: z.number(),
  previewUrl: z.string().nullable(),
  author: suggestionUserSchema.nullable(),
  mergedInto: suggestionRefSchema.nullable(),
  created: z.coerce.date(),
  updated: z.coerce.date(),
});

export const suggestionsSchema = z.object({
  suggestions: z.array(suggestionSchema),
});

export const suggestionAttachmentSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  mime: z.string(),
  size: z.number(),
  url: z.string(),
  created: z.coerce.date(),
});

export const suggestionMessageSchema = z.object({
  uuid: z.string(),
  author: suggestionUserSchema.nullable(),
  authorType: z.enum(['user', 'admin']),
  body: z.string(),
  created: z.coerce.date(),
});

export const suggestionThreadSchema = z.object({
  suggestion: suggestionSchema,
  messages: z.array(suggestionMessageSchema),
  attachments: z.array(suggestionAttachmentSchema),
});

export const suggestionCreateSchema = z.object({
  title: z.string().min(3).max(120),
  body: z.string().min(10).max(5000),
  serverType: suggestionServerType,
});

export const suggestionCommentSchema = z.object({
  body: z.string().min(1).max(5000),
});

export type SuggestionStatus = z.infer<typeof suggestionStatus>;
export type SuggestionServerType = z.infer<typeof suggestionServerType>;
export type Suggestion = z.infer<typeof suggestionSchema>;
export type SuggestionAttachment = z.infer<typeof suggestionAttachmentSchema>;
export type SuggestionMessage = z.infer<typeof suggestionMessageSchema>;
export type SuggestionThread = z.infer<typeof suggestionThreadSchema>;
