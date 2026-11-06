// plugged.host: Admin › Tickets (docs/features/tickets.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ticketSummarySchema } from '@/lib/schemas/user/tickets.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
  status?: string | null,
  category?: string | null,
  assigned?: string | null,
): Promise<Pagination<z.infer<typeof ticketSummarySchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/tickets', {
    params: {
      page,
      per_page: perPage,
      search,
      status: status ?? undefined,
      category: category ?? undefined,
      assigned: assigned ?? undefined,
    },
  });
  return parsePaginationFromApi(ticketSummarySchema, data.tickets);
};
