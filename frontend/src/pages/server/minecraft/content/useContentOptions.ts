// plugged.host: what the server can install: loader, Minecraft version, sources (docs/features/minecraft.md)
import getContentOptions from '@/api/server/minecraft/getContentOptions.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerStore } from '@/stores/server.ts';

export function useContentOptions() {
  const server = useServerStore((state) => state.server);

  return useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.content(),
    queryFn: () => getContentOptions(server.uuid),
  });
}
