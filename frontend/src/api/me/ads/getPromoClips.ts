// plugged.host: the house promo clips listed next to their files in `public/promo/` (docs/features/promos.md#promo-clips)
import { z } from 'zod';
import { promoClipSchema } from '@/lib/schemas/user/ads.ts';

/** The clips; none when the list is missing or unreadable, so no slot ever waits for one. */
export default async (): Promise<z.infer<typeof promoClipSchema>[]> => {
  try {
    const response = await fetch('/promo/index.json', { cache: 'no-cache' });
    if (!response.ok) return [];

    const list = await response.json();
    if (!Array.isArray(list)) return [];

    return list.flatMap((entry) => {
      const parsed = promoClipSchema.safeParse(entry);
      return parsed.success ? [parsed.data] : [];
    });
  } catch {
    return [];
  }
};
