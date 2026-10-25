// plugged.host: labels and colours of Admin › Partners' enums (docs/features/partners.md).
import { z } from 'zod';
import { adminPartnerFilter } from '@/lib/schemas/admin/partners.ts';
import { partnerStatus } from '@/lib/schemas/user/partners.ts';
import { getTranslations } from '@/providers/TranslationProvider.tsx';

type PartnerStatus = z.infer<typeof partnerStatus>;
type PartnerFilter = z.infer<typeof adminPartnerFilter>;

export const partnerStatusLabelMapping: Record<PartnerStatus, () => string> = {
  pending: () => getTranslations().t('pages.admin.partners.enum.status.pending', {}),
  approved: () => getTranslations().t('pages.admin.partners.enum.status.approved', {}),
  paused: () => getTranslations().t('pages.admin.partners.enum.status.paused', {}),
  suspended: () => getTranslations().t('pages.admin.partners.enum.status.suspended', {}),
  rejected: () => getTranslations().t('pages.admin.partners.enum.status.rejected', {}),
  revoked: () => getTranslations().t('pages.admin.partners.enum.status.revoked', {}),
  withdrawn: () => getTranslations().t('pages.admin.partners.enum.status.withdrawn', {}),
};

export const partnerStatusColorMapping: Record<PartnerStatus, string> = {
  pending: 'yellow',
  approved: 'green',
  paused: 'gray',
  suspended: 'red',
  rejected: 'gray',
  revoked: 'gray',
  withdrawn: 'gray',
};

export const partnerFilterOptions = () =>
  adminPartnerFilter.options.map((value: PartnerFilter) => ({
    value,
    label:
      value === 'partners'
        ? getTranslations().t('pages.admin.partners.enum.status.partners', {})
        : partnerStatusLabelMapping[value](),
  }));

/** What the bot's check found wrong, in an admin's words. */
export const partnerProblemLabel = (problem: string): string => {
  const { t } = getTranslations();

  switch (problem) {
    case 'bot_missing':
      return t('pages.admin.partners.enum.problem.botMissing', {});
    case 'too_few_members':
      return t('pages.admin.partners.enum.problem.tooFewMembers', {});
    case 'not_admin':
      return t('pages.admin.partners.enum.problem.notAdmin', {});
    case 'channel_missing':
      return t('pages.admin.partners.enum.problem.channelMissing', {});
    case 'channel_hidden':
      return t('pages.admin.partners.enum.problem.channelHidden', {});
    case 'channel_link':
      return t('pages.admin.partners.enum.problem.channelLink', {});
    default:
      return problem;
  }
};

export const partnerGameLabel = (game: string): string => {
  const { t } = getTranslations();

  switch (game) {
    case 'fivem':
      return t('pages.account.partners.games.fivem', {});
    case 'minecraft':
      return t('pages.account.partners.games.minecraft', {});
    default:
      return t('pages.account.partners.games.both', {});
  }
};
