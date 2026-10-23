import { z } from 'zod';
import { adminUpgradeStatusSchema } from '@/lib/schemas/admin/system.ts';

export type UpgradeAvailability = 'loading' | 'available' | 'upToDate' | 'unknown';

export function upgradeAvailability(
  status: z.infer<typeof adminUpgradeStatusSchema> | undefined,
  failed: boolean,
): UpgradeAvailability {
  if (!status) {
    return failed ? 'unknown' : 'loading';
  }
  if (status.updateAvailable) {
    return 'available';
  }

  return status.latestVersion ? 'upToDate' : 'unknown';
}
