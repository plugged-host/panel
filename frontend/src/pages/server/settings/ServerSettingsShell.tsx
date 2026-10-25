// plugged.host: the frame of every server settings page, the Pelican panel's settings page: the
// "Settings" title with a gray "Delete Server", the jump bar, then the page's sections in one
// column (docs/features/server-settings.md)
import { ReactNode, useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer, {
  Props as ServerContentContainerProps,
} from '@/elements/containers/ServerContentContainer.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import DeleteServerModal from './modals/DeleteServerModal.tsx';
import ServerSettingsNav from './ServerSettingsNav.tsx';
import { useCanDeleteServer } from './useSettingsSections.ts';

export default function ServerSettingsShell({
  registry,
  children,
}: {
  registry?: ServerContentContainerProps['registry'];
  children: ReactNode;
}) {
  const { t } = useTranslations();
  const canDelete = useCanDeleteServer();
  const [deleting, setDeleting] = useState(false);

  return (
    <ServerContentContainer
      title={t('pages.server.settings.title', {})}
      registry={registry}
      contentRight={
        canDelete ? (
          <Button variant='default' onClick={() => setDeleting(true)}>
            {t('pages.server.settings.delete.button', {})}
          </Button>
        ) : undefined
      }
    >
      {canDelete && <DeleteServerModal opened={deleting} onClose={() => setDeleting(false)} />}

      <div className='flex flex-col gap-6'>
        <ServerSettingsNav />
        {children}
      </div>
    </ServerContentContainer>
  );
}
