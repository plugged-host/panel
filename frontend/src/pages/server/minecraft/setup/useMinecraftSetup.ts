// plugged.host: a new Minecraft server's setup, which every page of the server sends to while it waits (docs/features/minecraft.md)
import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import getMinecraftOnboarding from '@/api/server/minecraft/getMinecraftOnboarding.ts';
import { isMinecraftServer } from '@/lib/domain/games.ts';
import { isInLimbo } from '@/lib/domain/limbo.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftOnboardingProgressSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useServerStore } from '@/stores/server.ts';

export const SETUP_PATH = '/minecraft/setup';

export function setupRunning(progress: z.infer<typeof minecraftOnboardingProgressSchema> | null | undefined) {
  return !!progress && progress.step !== 'done' && progress.step !== 'failed';
}

/** The setup of the server open now; only asked for Minecraft servers. */
export function useMinecraftSetup(enabled = true) {
  const server = useServerStore((state) => state.server);
  const canSetUp = useServerCan(['minecraft.settings', 'minecraft.players'], false);

  return useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.onboarding(),
    queryFn: () => getMinecraftOnboarding(server.uuid),
    enabled: enabled && !!server.uuid && canSetUp && isMinecraftServer(server.egg),
    retry: false,
    // applying the answers runs after their request: poll it until it ends
    refetchInterval: (query) => (setupRunning(query.state.data?.progress) ? 3000 : false),
  });
}

/**
 * Whether the server's pages wait for its setup: it is pending and the viewer may answer it (the
 * owner, an admin, a subuser with both permissions), and nothing else holds the server (a
 * suspension, a move, its node's maintenance, limbo). Every server page then sends to the setup and
 * the sidebar shows only it. `null` while that is not known yet.
 */
export function useSetupFirst(): boolean | null {
  const server = useServerStore((state) => state.server);
  const setup = useMinecraftSetup();

  if (setup.isLoading) return null;

  return (
    !!setup.data?.pending &&
    !server.isSuspended &&
    !server.isTransferring &&
    !server.nodeMaintenanceEnabled &&
    !isInLimbo(server.limboState)
  );
}
