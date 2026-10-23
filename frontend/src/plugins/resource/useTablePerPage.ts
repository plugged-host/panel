import { z } from 'zod';
import { useUserSetting } from '@/lib/userSettings.ts';

export const TABLE_PER_PAGE_KEY = 'table::per_page';

export const TABLE_PER_PAGE_OPTIONS = [10, 25, 50, 100] as const;
export const DEFAULT_TABLE_PER_PAGE = 25;

const tablePerPageSchema = z.number().int().min(1).max(100);

export function useTablePerPage() {
  return useUserSetting(TABLE_PER_PAGE_KEY, tablePerPageSchema, DEFAULT_TABLE_PER_PAGE);
}
