// plugged.host: the old settings pages' addresses land on their section of the one page (docs/features/server-settings.md)
import { Navigate, useLocation } from 'react-router';
import { useServerStore } from '@/stores/server.ts';

export default function ServerSettingsRedirect() {
  const { pathname } = useLocation();
  const server = useServerStore((state) => state.server);
  // the old page's last path segment is its section's anchor: /settings/sftp → /settings#sftp
  const section = pathname.split('/').filter(Boolean).pop();

  return <Navigate to={`/server/${server.uuidShort}/settings#${section}`} replace />;
}
