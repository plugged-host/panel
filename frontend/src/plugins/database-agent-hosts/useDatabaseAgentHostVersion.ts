import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import getDatabaseAgentHostSystemOverview from '@/api/admin/database-agent-hosts/getDatabaseAgentHostSystemOverview.ts';
import getDatabaseAgentHostUpgradeStatus from '@/api/admin/database-agent-hosts/getDatabaseAgentHostUpgradeStatus.ts';
import { UpgradeAvailability, upgradeAvailability } from '@/lib/domain/upgrade.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminDatabaseAgentHostSchema } from '@/lib/schemas/admin/databaseAgentHosts.ts';
import { adminUpgradeStatusSchema } from '@/lib/schemas/admin/system.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';

export function useDatabaseAgentHostUpgradeStatus(databaseAgentHostUuid: string, options?: { enabled?: boolean }) {
  const canReadDatabaseAgentHosts = useAdminCan('database-agent-hosts.read');

  const { data, error } = useQuery({
    queryKey: queryKeys.admin.databaseAgentHosts.upgradeStatus(databaseAgentHostUuid),
    queryFn: () => getDatabaseAgentHostUpgradeStatus(databaseAgentHostUuid),
    enabled: canReadDatabaseAgentHosts && (options?.enabled ?? true),
    staleTime: 5 * 60_000,
    retry: false,
  });

  return { status: data, availability: upgradeAvailability(data, !!error) };
}

interface UseDatabaseAgentHostVersionResult {
  version: string | null;
  unavailable: boolean;
  loading: boolean;
  upgradeAvailability: UpgradeAvailability;
}

export function useDatabaseAgentHostVersion(
  databaseAgentHost: z.infer<typeof adminDatabaseAgentHostSchema>,
  options?: { enabled?: boolean; upgradeStatus?: z.infer<typeof adminUpgradeStatusSchema> },
): UseDatabaseAgentHostVersionResult {
  const enabled = options?.enabled ?? true;

  const { data, error, loading } = useResource({
    queryKey: queryKeys.admin.databaseAgentHosts.systemOverview(databaseAgentHost.uuid),
    queryFn: () => getDatabaseAgentHostSystemOverview(databaseAgentHost.uuid),
    enabled,
    silent: true,
  });

  const fetched = useDatabaseAgentHostUpgradeStatus(databaseAgentHost.uuid, {
    enabled: enabled && !options?.upgradeStatus,
  });

  return {
    version: data?.version ?? null,
    unavailable: !!error,
    loading: loading && data === undefined && !error,
    upgradeAvailability: options?.upgradeStatus
      ? upgradeAvailability(options.upgradeStatus, false)
      : fetched.availability,
  };
}
