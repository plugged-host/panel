// plugged.host: SFTP access on the settings page (docs/features/server-settings.md)

import Button from '@/elements/buttons/Button.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Anchor from '@/elements/typography/Anchor.tsx';
import { useRedactedAddress } from '@/plugins/privacy/useRedactAddresses.ts';
import { useSftpPassword } from '@/plugins/useSftpPassword.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

export default function SftpContainer() {
  const { t } = useTranslations();
  const { user } = useAuth();
  const server = useServerStore((state) => state.server);
  const displayHost = useRedactedAddress(server.sftpHost);
  const password = useSftpPassword();
  const passwordLoginEnabled = useGlobalStore((state) => state.settings.app.passwordLoginEnabled);

  const address = `${server.sftpHost}:${server.sftpPort}`;
  const username = `${user!.username}.${server.uuidShort}`;

  return (
    <TitleCard title={t('pages.server.settings.sftp.title', {})} className='h-full order-12'>
      <Stack h='100%'>
        <CopyOnClick content={address} className='text-left'>
          <TextInput
            label={t('common.form.host', {})}
            value={`${displayHost}:${server.sftpPort}`}
            className='pointer-events-none'
            readOnly
          />
        </CopyOnClick>
        <CopyOnClick content={username} className='text-left'>
          <TextInput label={t('common.form.username', {})} value={username} className='pointer-events-none' readOnly />
        </CopyOnClick>
        <TextInput
          label={t('common.form.password', {})}
          value={
            password === 'noprofit'
              ? t('pages.server.settings.sftp.noprofitPassword', {})
              : password === 'panel' && passwordLoginEnabled
                ? t('common.form.yourControlPanelPassword', {})
                : t('pages.server.settings.sftp.sshKeyOnly', {})
          }
          className='pointer-events-none'
          readOnly
        />

        <Group mt='auto'>
          <Anchor href={`sftp://${username}@${address}`}>
            <Button variant='default'>{t('pages.server.files.modal.sftpDetails.launch', {})}</Button>
          </Anchor>
        </Group>
      </Stack>
    </TitleCard>
  );
}
