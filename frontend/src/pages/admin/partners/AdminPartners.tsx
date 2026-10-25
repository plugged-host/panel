// plugged.host: Admin › Partners: partner applications, neo's suggestions and the partnerships
// (docs/features/partners.md).
import { useState } from 'react';
import { z } from 'zod';
import getPartnerApplications from '@/api/admin/partners/getPartnerApplications.ts';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import Select from '@/elements/input/Select.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminPartnerFilter } from '@/lib/schemas/admin/partners.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import PartnerApplicationRow from './PartnerApplicationRow.tsx';
import { partnerFilterOptions } from './partnerLabels.ts';

type PartnerFilter = z.infer<typeof adminPartnerFilter>;

export default function AdminPartners() {
  const { t } = useTranslations();
  const [status, setStatus] = useState<PartnerFilter | null>(null);

  const {
    data: applications,
    loading,
    error,
    search,
    setSearch,
    setPage,
    setPerPage,
  } = useSearchablePaginatedTable({
    queryKey: [...queryKeys.admin.partners.all(), { status }],
    fetcher: (page, search, perPage) => getPartnerApplications(page, search, perPage, status),
    deps: [status],
  });

  return (
    <AdminContentContainer
      title={t('pages.admin.partners.title', {})}
      search={search}
      setSearch={setSearch}
      contentRight={
        <Select
          placeholder={t('pages.admin.partners.page.allStatuses', {})}
          value={status}
          onChange={(value) => setStatus(value ? adminPartnerFilter.parse(value) : null)}
          data={partnerFilterOptions()}
          clearable
          w={180}
        />
      }
    >
      <Table
        columns={[
          t('pages.admin.partners.page.column.server', {}),
          t('pages.admin.partners.page.column.applicant', {}),
          t('pages.admin.partners.page.column.members', {}),
          t('common.table.columns.status', {}),
          t('pages.admin.partners.page.column.neo', {}),
          t('pages.admin.partners.page.column.credits', {}),
          t('common.table.columns.created', {}),
          '',
        ]}
        loading={loading}
        error={error}
        pagination={applications}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {applications?.data.map((application) => (
          <PartnerApplicationRow key={application.uuid} application={application} />
        ))}
      </Table>
    </AdminContentContainer>
  );
}
