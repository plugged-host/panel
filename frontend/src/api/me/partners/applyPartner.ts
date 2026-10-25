// plugged.host: applies to the partner program; a refusal (422) carries what the check found
// (docs/features/partners.md)
import { isAxiosError } from 'axios';
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { partnerApplySchema, partnerRefusalSchema, partnersSchema } from '@/lib/schemas/user/partners.ts';
import { parseFromApi, safeParseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (application: z.infer<typeof partnerApplySchema>): Promise<z.infer<typeof partnersSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/partners', {
    server: application.server,
    game: application.game,
    description: application.description,
    servers: application.servers,
  });
  return parseFromApi(partnersSchema, data.partners);
};

/** What the check found when the panel refused an application, else `null`. */
export function partnerRefusal(error: unknown): z.infer<typeof partnerRefusalSchema> | null {
  if (!isAxiosError(error) || error.response?.status !== 422 || !error.response.data?.refusal) {
    return null;
  }
  const parsed = safeParseFromApi(partnerRefusalSchema, error.response.data.refusal);

  return parsed.success ? parsed.data : null;
}
