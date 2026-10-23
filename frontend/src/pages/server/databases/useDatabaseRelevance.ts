import { useQuery } from '@tanstack/react-query';
import getDatabaseHosts from '@/api/server/databases/getDatabaseHosts.ts';
import getDatabases from '@/api/server/databases/getDatabases.ts';
import getDatabaseInstances from '@/api/server/databases/instances/getDatabaseInstances.ts';
import getDatabaseInstanceTemplates from '@/api/server/databases/instances/getDatabaseInstanceTemplates.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { paginatedTableQueryKey } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTablePerPage } from '@/plugins/resource/useTablePerPage.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useServerStore } from '@/stores/server.ts';

export function useDatabaseRelevance() {
  const server = useServerStore((state) => state.server);

  const canReadClassic = useServerCan('databases.read');
  const canReadAgent = useServerCan('database-instances.read');
  const canCreateAgent = useServerCan('database-instances.create');
  const [perPage] = useTablePerPage();

  const { data: databases } = useQuery({
    queryKey: paginatedTableQueryKey(queryKeys.server(server.uuid).databases.all(), 1, '', perPage),
    queryFn: () => getDatabases(server.uuid, 1, '', perPage),
    enabled: canReadClassic,
  });

  const { data: instances } = useQuery({
    queryKey: paginatedTableQueryKey(queryKeys.server(server.uuid).databases.instances.all(), 1, '', perPage),
    queryFn: () => getDatabaseInstances(server.uuid, 1, '', perPage),
    enabled: canReadAgent,
  });

  const classicTotal = databases?.total ?? 0;
  const instanceTotal = instances?.total ?? 0;

  const used = classicTotal + instanceTotal;
  const full = used >= server.featureLimits.databases;

  const { data: databaseHosts } = useResource({
    queryKey: queryKeys.server(server.uuid).databases.hosts(),
    queryFn: () => getDatabaseHosts(server.uuid),
    enabled: canReadClassic && !full,
    silent: true,
  });

  const { data: agentTemplates } = useResource({
    queryKey: queryKeys.server(server.uuid).databases.instances.templates(),
    queryFn: () => getDatabaseInstanceTemplates(server.uuid),
    enabled: canCreateAgent && !full,
    silent: true,
  });

  const classicRelevant = canReadClassic && (classicTotal > 0 || (databaseHosts?.length ?? 0) > 0);
  const agentRelevant = canReadAgent && (instanceTotal > 0 || (agentTemplates?.length ?? 0) > 0);

  const settled =
    (!canReadClassic || databases !== undefined) &&
    (!canReadAgent || instances !== undefined) &&
    (!canReadClassic || full || databaseHosts !== undefined) &&
    (!canCreateAgent || full || agentTemplates !== undefined);

  return {
    canReadClassic,
    canReadAgent,
    classicTotal,
    instanceTotal,
    used,
    full,
    databaseHosts: databaseHosts ?? [],
    agentTemplates: agentTemplates ?? [],
    classicRelevant,
    agentRelevant,
    settled,
  };
}
