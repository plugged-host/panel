import getNodeSystemOverview from '@/api/admin/nodes/system/getNodeSystemOverview.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useOnlineWait } from '@/plugins/useOnlineWait.ts';

export function useNodeOnlineWait() {
  return useOnlineWait({
    queryKey: queryKeys.admin.nodes.systemOverview,
    queryFn: getNodeSystemOverview,
  });
}
