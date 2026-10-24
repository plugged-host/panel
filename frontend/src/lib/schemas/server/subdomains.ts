// plugged.host: a server's subdomains (docs/features/subdomains.md)
import { z } from 'zod';

export const serverSubdomainSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  domain: z.string(),
  fqdn: z.string(),
  recordType: z.string(),
  connect: z.string(),
  tokenCost: z.number(),
  propagating: z.boolean(),
  created: z.string(),
});

export const serverSubdomainsSchema = z.object({
  subdomains: z.array(serverSubdomainSchema),
  limit: z.number(),
  domains: z.array(z.object({ uuid: z.string(), name: z.string() })),
  service: z.string().nullable(),
  address: z.string().nullable(),
  unavailable: z.enum(['off', 'unsupported', 'no_domains', 'no_address']).nullable(),
  nextCost: z.number(),
  balance: z.number().nullable(),
});
