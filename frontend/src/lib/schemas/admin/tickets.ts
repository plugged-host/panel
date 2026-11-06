// plugged.host: Admin › Tickets (shared/src/tickets.rs, docs/features/tickets.md)
import { z } from 'zod';
import { ticketCategory, ticketStatus } from '@/lib/schemas/user/tickets.ts';

export const adminTicketStatusFilter = ticketStatus;
export const adminTicketCategoryFilter = ticketCategory;

export const adminTicketUpdateSchema = z.object({
  status: ticketStatus,
  priority: z.enum(['low', 'normal', 'high']),
});
