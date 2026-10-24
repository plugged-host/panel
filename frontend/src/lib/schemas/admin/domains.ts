// plugged.host: Admin › Domains, the Cloudflare zones server subdomains are made under (docs/features/subdomains.md)
import { z } from 'zod';

export const adminDomainSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  synced: z.boolean(),
  subdomains: z.number(),
  created: z.string(),
});

export const adminDomainsSchema = z.object({
  domains: z.array(adminDomainSchema),
  tokenSet: z.boolean(),
});
