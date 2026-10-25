// plugged.host: an admin's action on a partner application or partnership (docs/features/partners.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminPartnerApplicationSchema } from '@/lib/schemas/admin/partners.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export type PartnerAction =
  | { action: 'approve'; credits: number }
  | { action: 'reject'; reason: string }
  | { action: 'recheck' }
  | { action: 'revoke'; reason: string }
  | { action: 'pause'; reason: string }
  | { action: 'resume' }
  | { action: 'credits'; credits: number };

/** Runs the action and answers the application as it is now (`null` when it is gone). */
export default async (
  applicationUuid: string,
  action: PartnerAction,
): Promise<z.infer<typeof adminPartnerApplicationSchema> | null> => {
  const url = `/api/admin/partners/${applicationUuid}/${action.action}`;
  const { data } =
    action.action === 'credits'
      ? await axiosInstance.put(url, { credits: action.credits })
      : await axiosInstance.post(url, {
          ...('credits' in action ? { credits: action.credits } : {}),
          ...('reason' in action ? { reason: action.reason } : {}),
        });

  return data.application ? parseFromApi(adminPartnerApplicationSchema, data.application) : null;
};
