// plugged.host: Admin › Settings › AI › neo (docs/features/neo.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { parseJsonHeaders, parseJsonObject } from '@/lib/neo.ts';
import { adminSettingsNeoSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

const neoFieldsSchema = adminSettingsNeoSchema.omit({ providers: true });

export default async ({ providers, ...fields }: z.infer<typeof adminSettingsNeoSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    neo: {
      ...(serializeForApi(neoFieldsSchema, fields) as Record<string, unknown>),
      // the list replaces the slots
      providers: providers.map((provider) => ({
        slot: provider.slot,
        enabled: provider.enabled,
        label: provider.label,
        description: provider.description,
        driver: provider.driver,
        endpoint: provider.endpoint,
        // empty keeps the stored key, null removes it; a slot without one never takes another's
        api_key: provider.clearApiKey ? null : provider.apiKey.trim() || (provider.keyStored ? '' : null),
        model: provider.model,
        max_tokens: provider.maxTokens,
        timeout: provider.timeout,
        daily_token_cap: provider.dailyTokenCap,
        extra_headers: parseJsonHeaders(provider.extraHeaders) ?? {},
        extra_body: parseJsonObject(provider.extraBody) ?? {},
        access: provider.access,
        multiplier: provider.multiplier,
        vision: provider.vision === 'auto' ? null : provider.vision === 'yes',
      })),
    },
  });
};
