// plugged.host: Admin › Abuse: what the abuse checks found (docs/features/abuse.md).
import { faMagnifyingGlass } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { z } from 'zod';
import getAbuseFindings from '@/api/admin/abuse/getAbuseFindings.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import Select from '@/elements/input/Select.tsx';
import Group from '@/elements/layout/Group.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { abuseFindingStatus } from '@/lib/schemas/admin/abuse.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AbuseFindingRow from './AbuseFindingRow.tsx';
import { findingStatusOptions } from './abuseLabels.ts';
import AbuseScanModal from './modals/AbuseScanModal.tsx';

type FindingStatus = z.infer<typeof abuseFindingStatus>;

export default function AdminAbuse() {
  const { t } = useTranslations();
  const [status, setStatus] = useState<FindingStatus | null>('open');
  const [scanOpen, setScanOpen] = useState(false);

  const {
    data: findings,
    loading,
    error,
    search,
    setSearch,
    setPage,
    setPerPage,
  } = useSearchablePaginatedTable({
    queryKey: [...queryKeys.admin.abuse.findings(), { status }],
    fetcher: (page, search, perPage) => getAbuseFindings(page, search, perPage, status),
    deps: [status],
  });

  return (
    <AdminContentContainer
      title={t('pages.admin.abuse.title', {})}
      search={search}
      setSearch={setSearch}
      contentRight={
        <Group gap='xs'>
          <Select
            placeholder={t('pages.admin.abuse.page.allStatuses', {})}
            value={status}
            onChange={(value) => setStatus(value ? abuseFindingStatus.parse(value) : null)}
            data={findingStatusOptions()}
            clearable
            w={160}
          />
          <AdminCan action='abuse.manage'>
            <Button
              onClick={() => setScanOpen(true)}
              color='blue'
              leftSection={<FontAwesomeIcon icon={faMagnifyingGlass} />}
            >
              {t('pages.admin.abuse.page.button.scan', {})}
            </Button>
          </AdminCan>
        </Group>
      }
    >
      <AbuseScanModal opened={scanOpen} onClose={() => setScanOpen(false)} />

      <Table
        columns={[
          t('pages.admin.abuse.page.column.target', {}),
          t('pages.admin.abuse.page.column.category', {}),
          t('pages.admin.abuse.page.column.severity', {}),
          t('pages.admin.abuse.page.column.summary', {}),
          t('common.table.columns.status', {}),
          t('pages.admin.abuse.page.column.lastSeen', {}),
          '',
        ]}
        loading={loading}
        error={error}
        pagination={findings}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {findings?.data.map((finding) => (
          <AbuseFindingRow key={finding.uuid} finding={finding} />
        ))}
      </Table>
    </AdminContentContainer>
  );
}
