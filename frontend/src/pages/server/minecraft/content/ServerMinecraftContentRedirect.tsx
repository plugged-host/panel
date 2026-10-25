// plugged.host: the old Browse and Modpacks tabs are part of the one Plugins & Mods page now (docs/features/minecraft.md)
import { Navigate } from 'react-router';
import { useServerStore } from '@/stores/server.ts';

export default function ServerMinecraftContentRedirect() {
  const server = useServerStore((state) => state.server);

  return <Navigate to={`/server/${server.uuidShort}/minecraft/content`} replace />;
}
