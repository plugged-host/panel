// plugged.host: which password SFTP takes for the signed-in user: noprofit.space checks it when its
// account API is set up (backend/src/routes/api/remote/sftp/auth.rs), else the panel password.
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export function useSftpPassword(): 'noprofit' | 'panel' | 'none' {
  const { user } = useAuth();
  const sftpPasswords = useGlobalStore((state) => state.settings.noprofit.sftpPasswords);

  if (sftpPasswords) return 'noprofit';
  if (user?.hasPassword && !user.passwordLoginDisabled) return 'panel';

  return 'none';
}
