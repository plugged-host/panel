// plugged.host: Admin › Status › Overrides: forcing a component's status (maintenance, a known outage
// the checks don't see), for a time window or until removed.
import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import getStatusComponents from '@/api/admin/status/getStatusComponents.ts';
import getStatusOverrides from '@/api/admin/status/getStatusOverrides.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { onePage } from '../statusLabels.ts';
import OverrideCreateModal from './modals/OverrideCreateModal.tsx';
import StatusOverrideRow from './StatusOverrideRow.tsx';

export default function StatusOverrides() {
  const { t } = useTranslations();
  const [openModal, setOpenModal] = useState<'create' | null>(null);

  const overrides = useQuery({
    queryKey: queryKeys.admin.status.overrides(),
    queryFn: getStatusOverrides,
  });
  const components = useQuery({
    queryKey: queryKeys.admin.status.components(),
    queryFn: getStatusComponents,
  });

  return (
    <AdminSubContentContainer
      title={t('pages.admin.status.tabs.overrides.title', {})}
      titleOrder={2}
      contentRight={
        <AdminCan action='status.manage'>
          <Button onClick={() => setOpenModal('create')} color='blue' leftSection={<FontAwesomeIcon icon={faPlus} />}>
            {t('common.button.create', {})}
          </Button>
        </AdminCan>
      }
    >
      <OverrideCreateModal
        components={components.data ?? []}
        opened={openModal === 'create'}
        onClose={() => setOpenModal(null)}
      />

      <Table
        columns={[
          t('pages.admin.status.tabs.overrides.page.column.component', {}),
          t('common.table.columns.status', {}),
          t('common.table.columns.notes', {}),
          t('pages.admin.status.tabs.overrides.page.column.starts', {}),
          t('pages.admin.status.tabs.overrides.page.column.ends', {}),
          t('pages.admin.status.tabs.overrides.page.column.active', {}),
          '',
        ]}
        loading={overrides.isLoading}
        error={overrides.error ? httpErrorToHuman(overrides.error) : null}
        pagination={onePage(overrides.data)}
      >
        {overrides.data?.map((override) => (
          <StatusOverrideRow key={override.uuid} override={override} components={components.data ?? []} />
        ))}
      </Table>
    </AdminSubContentContainer>
  );
}
