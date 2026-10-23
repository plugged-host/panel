import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import getNodeUpgradeStatus from '@/api/admin/nodes/getNodeUpgradeStatus.ts';
import getNodeSystemDirect from '@/api/admin/nodes/system/getNodeSystemDirect.ts';
import { UpgradeAvailability, upgradeAvailability } from '@/lib/domain/upgrade.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminNode } from '@/lib/schemas/admin/nodes.ts';
import { adminUpgradeStatusSchema } from '@/lib/schemas/admin/system.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';

export function useNodeUpgradeStatus(nodeUuid: string, options?: { enabled?: boolean }) {
  const canReadNodes = useAdminCan('nodes.read');

  const { data, error } = useQuery({
    queryKey: queryKeys.admin.nodes.upgradeStatus(nodeUuid),
    queryFn: () => getNodeUpgradeStatus(nodeUuid),
    enabled: canReadNodes && (options?.enabled ?? true),
    staleTime: 5 * 60_000,
    retry: false,
  });

  return { status: data, availability: upgradeAvailability(data, !!error) };
}

interface UseNodeVersionResult {
  version: string | null;
  unavailable: boolean;
  loading: boolean;
  upgradeAvailability: UpgradeAvailability;
}

export function useNodeVersion(
  node: AdminNode,
  options?: { enabled?: boolean; upgradeStatus?: z.infer<typeof adminUpgradeStatusSchema> },
): UseNodeVersionResult {
  const enabled = options?.enabled ?? true;

  const { data, error, loading } = useResource({
    queryKey: [...queryKeys.admin.nodes.systemDirect(node.uuid), node.publicUrl ?? node.url],
    queryFn: () => getNodeSystemDirect(node),
    enabled,
    silent: true,
  });

  const fetched = useNodeUpgradeStatus(node.uuid, { enabled: enabled && !options?.upgradeStatus });

  return {
    version: data?.version ?? null,
    unavailable: !!error,
    loading: loading && data === undefined && !error,
    upgradeAvailability: options?.upgradeStatus
      ? upgradeAvailability(options.upgradeStatus, false)
      : fetched.availability,
  };
}
