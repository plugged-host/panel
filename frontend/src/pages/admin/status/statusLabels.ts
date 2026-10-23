// plugged.host: labels and colours of Admin › Status's enums.
import { z } from 'zod';
import { statusComponentStatus, statusIncidentImpact, statusIncidentStatus } from '@/lib/schemas/admin/status.ts';
import { getTranslations } from '@/providers/TranslationProvider.tsx';

type ComponentStatus = z.infer<typeof statusComponentStatus>;
type IncidentImpact = z.infer<typeof statusIncidentImpact>;
type IncidentStatus = z.infer<typeof statusIncidentStatus>;

export const componentStatusLabelMapping: Record<ComponentStatus, () => string> = {
  operational: () => getTranslations().t('pages.admin.status.enum.componentStatus.operational', {}),
  degraded: () => getTranslations().t('pages.admin.status.enum.componentStatus.degraded', {}),
  maintenance: () => getTranslations().t('pages.admin.status.enum.componentStatus.maintenance', {}),
  partial_outage: () => getTranslations().t('pages.admin.status.enum.componentStatus.partialOutage', {}),
  major_outage: () => getTranslations().t('pages.admin.status.enum.componentStatus.majorOutage', {}),
  offline: () => getTranslations().t('pages.admin.status.enum.componentStatus.offline', {}),
  unknown: () => getTranslations().t('pages.admin.status.enum.componentStatus.unknown', {}),
};

export const componentStatusColorMapping: Record<ComponentStatus, string> = {
  operational: 'green',
  degraded: 'yellow',
  maintenance: 'blue',
  partial_outage: 'orange',
  major_outage: 'red',
  offline: 'red',
  unknown: 'gray',
};

export const incidentImpactLabelMapping: Record<IncidentImpact, () => string> = {
  minor: () => getTranslations().t('pages.admin.status.enum.incidentImpact.minor', {}),
  major: () => getTranslations().t('pages.admin.status.enum.incidentImpact.major', {}),
  critical: () => getTranslations().t('pages.admin.status.enum.incidentImpact.critical', {}),
  maintenance: () => getTranslations().t('pages.admin.status.enum.incidentImpact.maintenance', {}),
};

export const incidentImpactColorMapping: Record<IncidentImpact, string> = {
  minor: 'yellow',
  major: 'orange',
  critical: 'red',
  maintenance: 'blue',
};

export const incidentStatusLabelMapping: Record<IncidentStatus, () => string> = {
  investigating: () => getTranslations().t('pages.admin.status.enum.incidentStatus.investigating', {}),
  identified: () => getTranslations().t('pages.admin.status.enum.incidentStatus.identified', {}),
  monitoring: () => getTranslations().t('pages.admin.status.enum.incidentStatus.monitoring', {}),
  resolved: () => getTranslations().t('pages.admin.status.enum.incidentStatus.resolved', {}),
};

export const componentStatusOptions = () =>
  statusComponentStatus.options.map((value) => ({ value, label: componentStatusLabelMapping[value]() }));
export const incidentImpactOptions = () =>
  statusIncidentImpact.options.map((value) => ({ value, label: incidentImpactLabelMapping[value]() }));
export const incidentStatusOptions = () =>
  statusIncidentStatus.options.map((value) => ({ value, label: incidentStatusLabelMapping[value]() }));

/** An unpaginated list as one page, so the table shows its empty state. */
export const onePage = <T>(items: T[] | undefined): Pagination<T> | undefined =>
  items && { total: items.length, perPage: Math.max(items.length, 1), page: 1, data: items };
