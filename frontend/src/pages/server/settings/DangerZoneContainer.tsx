// plugged.host: the Pelican panel's "Danger Zone" section (ServerSettings::dangerZoneSection,
// settings/danger-zone.blade.php): the red-edged section with Reinstall and Delete Server, both
// gray until their confirmation (docs/features/server-settings.md)
import { useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import DeleteServerModal from './modals/DeleteServerModal.tsx';
import SettingsReinstallModal from './modals/SettingsReinstallModal.tsx';
import { sectionAnchor, useCanDeleteServer } from './useSettingsSections.ts';

const base = 'pages.server.settings';

export default function DangerZoneContainer() {
  const { t } = useTranslations();
  const canReinstall = useServerCan('settings.install');
  const canDelete = useCanDeleteServer();
  const [modal, setModal] = useState<'reinstall' | 'delete' | null>(null);

  if (!canReinstall && !canDelete) {
    return null;
  }

  return (
    <TitleCard
      {...sectionAnchor('danger')}
      danger
      title={t(`${base}.danger.title`, {})}
      footer={
        <>
          {canReinstall && (
            <Button variant='default' onClick={() => setModal('reinstall')}>
              {t(`${base}.danger.reinstallButton`, {})}
            </Button>
          )}
          {canDelete && (
            <Button variant='default' onClick={() => setModal('delete')}>
              {t(`${base}.delete.button`, {})}
            </Button>
          )}
        </>
      }
    >
      {canReinstall && <SettingsReinstallModal opened={modal === 'reinstall'} onClose={() => setModal(null)} />}
      {canDelete && <DeleteServerModal opened={modal === 'delete'} onClose={() => setModal(null)} />}

      <p className='theme-setting-note'>{t(`${base}.danger.note`, {})}</p>
      <div className='theme-setting-danger-rows'>
        {canReinstall && (
          <div className='theme-setting-danger-row'>
            <strong>{t(`${base}.danger.reinstallTitle`, {})}</strong>
            <p>{t(`${base}.danger.reinstallBody`, {})}</p>
          </div>
        )}
        {canDelete && (
          <div className='theme-setting-danger-row'>
            <strong>{t(`${base}.delete.title`, {})}</strong>
            <p>{t(`${base}.danger.deleteBody`, {})}</p>
          </div>
        )}
      </div>
    </TitleCard>
  );
}
