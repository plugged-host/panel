// plugged.host: Admin › Status › Incidents
import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import getStatusComponents from '@/api/admin/status/getStatusComponents.ts';
import getStatusIncidents from '@/api/admin/status/getStatusIncidents.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { onePage } from '../statusLabels.ts';
import IncidentCreateModal from './modals/IncidentCreateModal.tsx';
import StatusIncidentRow from './StatusIncidentRow.tsx';

export default function StatusIncidents() {
  const { t } = useTranslations();
  const [openModal, setOpenModal] = useState<'create' | null>(null);

  const incidents = useQuery({
    queryKey: queryKeys.admin.status.incidents(),
    queryFn: getStatusIncidents,
  });
  const components = useQuery({
    queryKey: queryKeys.admin.status.components(),
    queryFn: getStatusComponents,
  });

  return (
    <AdminSubContentContainer
      title={t('pages.admin.status.tabs.incidents.title', {})}
      titleOrder={2}
      contentRight={
        <AdminCan action='status.manage'>
          <Button onClick={() => setOpenModal('create')} color='blue' leftSection={<FontAwesomeIcon icon={faPlus} />}>
            {t('common.button.create', {})}
          </Button>
        </AdminCan>
      }
    >
      <IncidentCreateModal
        components={components.data ?? []}
        opened={openModal === 'create'}
        onClose={() => setOpenModal(null)}
      />

      <Table
        columns={[
          t('common.table.columns.title', {}),
          t('pages.admin.status.tabs.incidents.page.column.impact', {}),
          t('common.table.columns.status', {}),
          t('pages.admin.status.tabs.incidents.page.column.components', {}),
          t('pages.admin.status.tabs.incidents.page.column.started', {}),
          t('pages.admin.status.tabs.incidents.page.column.resolved', {}),
          '',
        ]}
        loading={incidents.isLoading}
        error={incidents.error ? httpErrorToHuman(incidents.error) : null}
        pagination={onePage(incidents.data)}
      >
        {incidents.data?.map((incident) => (
          <StatusIncidentRow key={incident.uuid} incident={incident} components={components.data ?? []} />
        ))}
      </Table>
    </AdminSubContentContainer>
  );
}
