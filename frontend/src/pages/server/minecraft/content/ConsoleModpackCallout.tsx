// plugged.host: a modpack's install on the console, where the server's reinstall for it sends the player (docs/features/minecraft.md#modpacks)
import { useEffect, useState } from 'react';
import { isMinecraftServer } from '@/lib/domain/games.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useServerStore } from '@/stores/server.ts';
import { setupRunning, useMinecraftSetup } from '../setup/useMinecraftSetup.ts';
import ModpackInstallCallout from './ModpackInstallCallout.tsx';
import { modpackInstalling, useModpacks } from './useModpacks.ts';

export default function ConsoleModpackCallout() {
  const server = useServerStore((state) => state.server);
  const canContent = useServerCan('minecraft.content');
  const setup = useMinecraftSetup();
  // asked while the server installs, and followed to its end once it was seen running (also one
  // Plugins & Mods already knows of)
  const [follow, setFollow] = useState(false);

  // a setup shows its own pack with its progress
  const setupShows = setup.isLoading || !!setup.data?.pending || setupRunning(setup.data?.progress);
  const allowed = canContent && isMinecraftServer(server.egg) && !setupShows;
  const modpacks = useModpacks(allowed && (server.status === 'installing' || follow));
  const running = modpackInstalling(modpacks.data?.install);

  useEffect(() => {
    if (running) setFollow(true);
  }, [running]);

  if (!allowed || !(follow || running) || !modpacks.data?.install) return null;

  return (
    <div className='mb-6'>
      <ModpackInstallCallout enabled={false} />
    </div>
  );
}
