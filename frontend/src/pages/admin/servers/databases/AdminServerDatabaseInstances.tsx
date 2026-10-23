import getServerDatabaseInstances from '@/api/admin/servers/databases/getServerDatabaseInstances.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminServer } from '@/lib/schemas/admin/servers.ts';
import { serverDatabaseInstanceTableColumns } from '@/lib/tableColumns.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AdminServerDatabaseInstanceRow from './AdminServerDatabaseInstanceRow.tsx';

export default function AdminServerDatabaseInstances({ server }: { server: AdminServer }) {
  const { t } = useTranslations();
  const {
    data: instances,
    loading,
    error,
    search,
    setSearch,
    setPage,
    setPerPage,
  } = useSearchablePaginatedTable({
    queryKey: queryKeys.admin.databaseInstances.byServer(server.uuid),
    fetcher: (page, search, perPage) => getServerDatabaseInstances(server.uuid, page, search, perPage),
  });

  return (
    <AdminSubContentContainer
      title={t('pages.admin.servers.tabs.databases.page.instances.title', {})}
      titleOrder={2}
      search={search}
      setSearch={setSearch}
      registry={window.extensionContext.extensionRegistry.pages.admin.servers.view.databases.instancesSubContainer}
      registryProps={{ server }}
    >
      <Table
        columns={serverDatabaseInstanceTableColumns()}
        loading={loading}
        error={error}
        pagination={instances}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {instances?.data.map((databaseAgent) => (
          <AdminServerDatabaseInstanceRow key={databaseAgent.uuid} server={server} databaseAgent={databaseAgent} />
        ))}
      </Table>
    </AdminSubContentContainer>
  );
}
