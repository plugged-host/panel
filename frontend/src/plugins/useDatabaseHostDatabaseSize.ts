import getDatabaseHostDatabaseSizes from '@/api/admin/database-hosts/getDatabaseHostDatabaseSizes.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';

export function useDatabaseHostDatabaseSize(hostUuid: string, databaseUuid: string) {
  const { data, loading } = useResource({
    queryKey: queryKeys.admin.databaseHosts.databaseSizes(hostUuid),
    queryFn: () => getDatabaseHostDatabaseSizes(hostUuid),
    silent: true,
  });

  return {
    size: data?.find((database) => database.uuid === databaseUuid)?.size,
    loading: loading && data === undefined,
  };
}
