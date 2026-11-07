// plugged.host: support tickets (shared/src/tickets.rs, docs/features/tickets.md)
import { z } from 'zod';

export const ticketStatus = z.enum(['open', 'waiting', 'resolved', 'closed']);
export const ticketCategory = z.enum(['general', 'billing', 'technical', 'bug', 'abuse', 'other']);
export const ticketPriority = z.enum(['low', 'normal', 'high']);
export const ticketAuthorType = z.enum(['user', 'admin', 'neo', 'system']);

export const ticketUserSchema = z.object({
  uuid: z.string(),
  username: z.string(),
});

export const ticketServerSchema = z.object({
  uuid: z.string(),
  name: z.string(),
});

export const ticketBugSchema = z.object({
  steps: z.string().nullable(),
  expected: z.string().nullable(),
  actual: z.string().nullable(),
  environment: z.string().nullable(),
});

export const ticketSummarySchema = z.object({
  uuid: z.string(),
  subject: z.string(),
  category: ticketCategory,
  status: ticketStatus,
  priority: ticketPriority,
  server: ticketServerSchema.nullable(),
  assignedTo: ticketUserSchema.nullable(),
  messages: z.number(),
  lastMessage: z.coerce.date().nullable(),
  created: z.coerce.date(),
  updated: z.coerce.date(),
  closed: z.coerce.date().nullable(),
  user: ticketUserSchema.nullable(),
  bug: ticketBugSchema.nullable(),
});

export const ticketAttachmentSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  mime: z.string(),
  size: z.number(),
  url: z.string(),
  created: z.coerce.date(),
});

export const ticketMessageSchema = z.object({
  uuid: z.string(),
  author: ticketUserSchema.nullable(),
  authorType: ticketAuthorType,
  body: z.string(),
  created: z.coerce.date(),
});

export const ticketSchema = z.object({
  ticket: ticketSummarySchema,
  messages: z.array(ticketMessageSchema),
  attachments: z.array(ticketAttachmentSchema),
});

export const ticketsSchema = z.object({
  tickets: z.array(ticketSummarySchema),
});

export const ticketCreateSchema = z.object({
  subject: z.string().min(3).max(200),
  category: ticketCategory,
  serverUuid: z.string().nullable(),
  body: z.string().min(1).max(20000),
  bugSteps: z.string().max(20000).optional(),
  bugExpected: z.string().max(20000).optional(),
  bugActual: z.string().max(20000).optional(),
  bugEnvironment: z.string().max(2000).optional(),
});

export const ticketReplySchema = z.object({
  body: z.string().min(1).max(20000),
});

export type TicketStatus = z.infer<typeof ticketStatus>;
export type TicketCategory = z.infer<typeof ticketCategory>;
export type TicketPriority = z.infer<typeof ticketPriority>;
export type TicketSummary = z.infer<typeof ticketSummarySchema>;
export type TicketMessage = z.infer<typeof ticketMessageSchema>;
export type Ticket = z.infer<typeof ticketSchema>;
