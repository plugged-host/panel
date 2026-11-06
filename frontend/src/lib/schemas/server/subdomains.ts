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

export const serverCustomDomainSchema = z.object({
  uuid: z.string(),
  domain: z.string(),
  target: z.string(),
  verificationToken: z.string(),
  verificationName: z.string(),
  verifiedAt: z.coerce.date().nullable(),
  created: z.coerce.date(),
});

export const serverSubdomainsSchema = z.object({
  subdomains: z.array(serverSubdomainSchema),
  limit: z.number(),
  domains: z.array(z.object({ uuid: z.string(), name: z.string() })),
  // plugged.host: the person's own domains, pointed at the panel subdomain or address
  customDomains: z.array(serverCustomDomainSchema).default([]),
  customTarget: z.string().nullable().default(null),
  service: z.string().nullable(),
  address: z.string().nullable(),
  // discord_link / discord_member: the Discord settings' gate (docs/features/discord.md)
  unavailable: z.enum(['off', 'no_domains', 'no_address', 'discord_link', 'discord_member']).nullable(),
  nextCost: z.number(),
  // the least deposit a name of 2 and of 3 characters takes (docs/features/server-upgrades.md)
  shortTwo: z.number().default(0),
  shortThree: z.number().default(0),
  balance: z.number().nullable(),
});
