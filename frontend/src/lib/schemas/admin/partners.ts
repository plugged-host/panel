// plugged.host: Admin › Partners (shared/src/partners.rs, docs/features/partners.md)
import { z } from 'zod';
import {
  partnerChannelSchema,
  partnerPaymentSchema,
  partnerServerSchema,
  partnerStatus,
} from '@/lib/schemas/user/partners.ts';

export const adminPartnerFactsSchema = z.object({
  name: z.string(),
  iconUrl: z.string().nullable(),
  description: z.string().nullable(),
  members: z.number(),
  online: z.number(),
  created: z.coerce.date().nullable(),
  boostTier: z.number(),
  boosts: z.number(),
  features: z.array(z.string()),
  verificationLevel: z.number(),
  locale: z.string().nullable(),
  standing: z.enum(['owner', 'administrator', 'member', 'absent']),
  channel: partnerChannelSchema.nullable(),
});

export const adminPartnerAssessmentSchema = z.object({
  accept: z.boolean(),
  credits: z.number(),
  suggested: z.number().nullable(),
  reasons: z.array(z.string()),
  model: z.string().nullable(),
});

export const adminPartnerApplicationSchema = z.object({
  uuid: z.string(),
  user: z
    .object({
      uuid: z.string(),
      username: z.string(),
      created: z.coerce.date(),
    })
    .nullable(),
  discordId: z.string(),
  guildId: z.string(),
  guildName: z.string(),
  inviteUrl: z.string().nullable(),
  game: z.string(),
  description: z.string(),
  servers: z.array(partnerServerSchema),
  facts: adminPartnerFactsSchema,
  problems: z.array(z.string()),
  checked: z.coerce.date(),
  assessment: adminPartnerAssessmentSchema.nullable(),
  // why neo has no assessment (shared/src/partners.rs, assessment_error); an unknown one reads as unavailable
  assessmentError: z.enum(['off', 'unavailable', 'busy', 'unreadable']).nullable().catch('unavailable'),
  assessed: z.coerce.date().nullable(),
  status: partnerStatus,
  monthlyCredits: z.number(),
  nextPayment: z.coerce.date().nullable(),
  failingSince: z.coerce.date().nullable(),
  suspendAfter: z.coerce.date().nullable(),
  payments: z.array(partnerPaymentSchema),
  paidTotal: z.number(),
  reason: z.string().nullable(),
  decidedBy: z.string().nullable(),
  decided: z.coerce.date().nullable(),
  approved: z.coerce.date().nullable(),
  created: z.coerce.date(),
});

/** The list's filter: a status, or every partnership. */
export const adminPartnerFilter = z.enum([
  'pending',
  'partners',
  'approved',
  'paused',
  'suspended',
  'rejected',
  'revoked',
  'withdrawn',
]);

export type AdminPartnerApplication = z.infer<typeof adminPartnerApplicationSchema>;
