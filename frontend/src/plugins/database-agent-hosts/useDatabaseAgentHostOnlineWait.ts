import getDatabaseAgentHostSystemOverview from '@/api/admin/database-agent-hosts/getDatabaseAgentHostSystemOverview.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useOnlineWait } from '@/plugins/useOnlineWait.ts';

export function useDatabaseAgentHostOnlineWait() {
  return useOnlineWait({
    queryKey: queryKeys.admin.databaseAgentHosts.systemOverview,
    queryFn: getDatabaseAgentHostSystemOverview,
  });
}
