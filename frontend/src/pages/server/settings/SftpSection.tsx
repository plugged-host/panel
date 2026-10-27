// plugged.host: Settings' "SFTP Information" rows: the connection, the username and which password works,
// with copy buttons and the sftp:// link, as the VS Code page lists its own (docs/features/server-settings.md#sftp)
import { CopyActionIcon, LinkActionIcon, SettingRow, SettingRows } from '@/elements/SettingRows.tsx';
import { useRedactedAddress } from '@/plugins/privacy/useRedactAddresses.ts';
import { useSftpPassword } from '@/plugins/useSftpPassword.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';
import SettingsSection from './SettingsSection.tsx';

const base = 'pages.server.settings.sftp';

export default function SftpSection() {
  const { t } = useTranslations();
  const { user } = useAuth();
  const server = useServerStore((state) => state.server);
  const displayHost = useRedactedAddress(server.sftpHost);
  const password = useSftpPassword();
  const passwordLoginEnabled = useGlobalStore((state) => state.settings.app.passwordLoginEnabled);

  const address = `${server.sftpHost}:${server.sftpPort}`;
  const username = `${user!.username}.${server.uuidShort}`;
  // noprofit.space checks the account's password; else the panel's, while password sign-in is on
  const accountPassword = password === 'noprofit' || (password === 'panel' && passwordLoginEnabled);

  return (
    <SettingsSection id='sftp' title={t(`${base}.title`, {})} description={t(`${base}.description`, {})}>
      <SettingRows className='theme-eco-list'>
        <SettingRow
          label={t(`${base}.connection`, {})}
          mono
          actions={
            <>
              <CopyActionIcon content={address} label={t(`${base}.copyHost`, {})} />
              <LinkActionIcon href={`sftp://${username}@${address}`} label={t(`${base}.launch`, {})} />
            </>
          }
        >
          {`${displayHost}:${server.sftpPort}`}
        </SettingRow>
        <SettingRow
          label={t(`${base}.username`, {})}
          mono
          actions={<CopyActionIcon content={username} label={t(`${base}.copyUsername`, {})} />}
        >
          {username}
        </SettingRow>
        <SettingRow
          label={t(`${base}.password`, {})}
          hint={accountPassword ? t(`${base}.passwordHint`, {}) : t(`${base}.sshKeyHint`, {})}
        >
          {accountPassword ? t(`${base}.accountPassword`, {}) : t(`${base}.sshKey`, {})}
        </SettingRow>
      </SettingRows>
    </SettingsSection>
  );
}
