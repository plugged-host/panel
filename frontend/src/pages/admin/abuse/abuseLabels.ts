// plugged.host: labels and colours of Admin › Abuse's enums.
import { z } from 'zod';
import { abuseFindingStatus, abuseSeverity } from '@/lib/schemas/admin/abuse.ts';
import { getTranslations } from '@/providers/TranslationProvider.tsx';

type FindingStatus = z.infer<typeof abuseFindingStatus>;
type Severity = z.infer<typeof abuseSeverity>;

export const findingStatusLabelMapping: Record<FindingStatus, () => string> = {
  open: () => getTranslations().t('pages.admin.abuse.enum.status.open', {}),
  actioned: () => getTranslations().t('pages.admin.abuse.enum.status.actioned', {}),
  dismissed: () => getTranslations().t('pages.admin.abuse.enum.status.dismissed', {}),
  resolved: () => getTranslations().t('pages.admin.abuse.enum.status.resolved', {}),
};

export const findingStatusColorMapping: Record<FindingStatus, string> = {
  open: 'yellow',
  actioned: 'red',
  dismissed: 'gray',
  resolved: 'green',
};

export const severityLabelMapping: Record<Severity, () => string> = {
  critical: () => getTranslations().t('pages.admin.abuse.enum.severity.critical', {}),
  review: () => getTranslations().t('pages.admin.abuse.enum.severity.review', {}),
};

export const severityColorMapping: Record<Severity, string> = {
  critical: 'red',
  review: 'yellow',
};

export const findingStatusOptions = () =>
  (Object.keys(findingStatusLabelMapping) as FindingStatus[]).map((value) => ({
    value,
    label: findingStatusLabelMapping[value](),
  }));
