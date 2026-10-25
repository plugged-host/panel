// plugged.host: what the server can take as a modpack and the install in progress (docs/features/minecraft.md)
import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import getModpacks from '@/api/server/minecraft/getModpacks.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftModpackInstallSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerStore } from '@/stores/server.ts';

export type ModpackInstall = z.infer<typeof minecraftModpackInstallSchema>;

/** The install has not ended yet (it runs after its request). */
export function modpackInstalling(install: ModpackInstall | null | undefined) {
  return !!install && install.step !== 'done' && install.step !== 'failed';
}

/** Polled every 3 seconds while a pack is being installed. */
export function useModpacks(enabled = true) {
  const server = useServerStore((state) => state.server);

  return useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.modpacks(),
    queryFn: () => getModpacks(server.uuid),
    enabled,
    refetchInterval: (query) => (modpackInstalling(query.state.data?.install) ? 3000 : false),
  });
}
