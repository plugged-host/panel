// plugged.host: Admin › Status › Components: the website, the panel and every node, as the status
// page lists them.
import { useQuery } from '@tanstack/react-query';
import getStatusComponents from '@/api/admin/status/getStatusComponents.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { onePage } from '../statusLabels.ts';
import StatusComponentRow from './StatusComponentRow.tsx';

export default function StatusComponents() {
  const { t } = useTranslations();

  const components = useQuery({
    queryKey: queryKeys.admin.status.components(),
    queryFn: getStatusComponents,
    refetchInterval: 60_000,
  });

  return (
    <AdminSubContentContainer title={t('pages.admin.status.tabs.components.title', {})} titleOrder={2}>
      <Table
        columns={[
          t('common.table.columns.name', {}),
          t('common.table.columns.type', {}),
          t('common.table.columns.status', {}),
          t('pages.admin.status.tabs.components.page.column.published', {}),
          t('pages.admin.status.tabs.components.page.column.sort', {}),
          t('pages.admin.status.tabs.components.page.column.checked', {}),
          '',
        ]}
        loading={components.isLoading}
        error={components.error ? httpErrorToHuman(components.error) : null}
        pagination={onePage(components.data)}
        empty={t('pages.admin.status.tabs.components.page.empty', {})}
      >
        {components.data?.map((component) => (
          <StatusComponentRow key={component.uuid} component={component} />
        ))}
      </Table>
    </AdminSubContentContainer>
  );
}
