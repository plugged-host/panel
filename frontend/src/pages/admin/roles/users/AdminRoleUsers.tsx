import { z } from 'zod';
import getRoleUsers from '@/api/admin/roles/users/getRoleUsers.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { roleSchema } from '@/lib/schemas/user.ts';
import { userTableColumns } from '@/lib/tableColumns.ts';
import UserRow from '@/pages/admin/users/UserRow.tsx';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function AdminRoleUsers({ role }: { role: z.infer<typeof roleSchema> }) {
  const { t } = useTranslations();
  const {
    data: roleUsers,
    loading,
    error,
    search,
    setSearch,
    setPage,
    setPerPage,
  } = useSearchablePaginatedTable({
    queryKey: queryKeys.admin.roles.users(role.uuid),
    fetcher: (page, search, perPage) => getRoleUsers(role.uuid, page, search, perPage),
  });

  return (
    <AdminSubContentContainer
      title={t('pages.admin.roles.tabs.users.page.title', {})}
      titleOrder={2}
      search={search}
      setSearch={setSearch}
      registry={window.extensionContext.extensionRegistry.pages.admin.roles.view.users.subContainer}
      registryProps={{ role }}
    >
      <Table
        columns={userTableColumns()}
        loading={loading}
        error={error}
        pagination={roleUsers}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {roleUsers?.data.map((user) => (
          <UserRow key={user.uuid} user={user} />
        ))}
      </Table>
    </AdminSubContentContainer>
  );
}
