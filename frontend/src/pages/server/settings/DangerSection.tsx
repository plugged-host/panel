// plugged.host: Settings' last section, "Danger Zone": Reinstall Server and Delete Server as rows, what each
// does beside a gray button; the red is only in their confirmations (docs/features/server-settings.md#danger-zone)
import { useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import DeleteServerModal from './modals/DeleteServerModal.tsx';
import SettingsReinstallModal from './modals/SettingsReinstallModal.tsx';
import SettingsSection from './SettingsSection.tsx';
import { useCanDeleteServer } from './useSettingsSections.ts';

const base = 'pages.server.settings';

export default function DangerSection() {
  const { t } = useTranslations();
  const canReinstall = useServerCan('settings.install');
  const canDelete = useCanDeleteServer();
  const [modal, setModal] = useState<'reinstall' | 'delete' | null>(null);

  if (!canReinstall && !canDelete) {
    return null;
  }

  return (
    <SettingsSection id='danger' title={t(`${base}.danger.title`, {})} description={t(`${base}.danger.note`, {})}>
      {canReinstall && <SettingsReinstallModal opened={modal === 'reinstall'} onClose={() => setModal(null)} />}
      {canDelete && <DeleteServerModal opened={modal === 'delete'} onClose={() => setModal(null)} />}

      <div className='theme-eco-card'>
        <div className='theme-rows'>
          {canReinstall && (
            <div className='theme-row'>
              <div className='theme-row-text'>
                <h3 className='theme-row-title'>{t(`${base}.danger.reinstallTitle`, {})}</h3>
                <p className='theme-row-body'>{t(`${base}.danger.reinstallBody`, {})}</p>
              </div>
              <div className='theme-row-actions'>
                <Button variant='default' size='sm' onClick={() => setModal('reinstall')}>
                  {t(`${base}.danger.reinstallButton`, {})}
                </Button>
              </div>
            </div>
          )}
          {canDelete && (
            <div className='theme-row'>
              <div className='theme-row-text'>
                <h3 className='theme-row-title'>{t(`${base}.delete.title`, {})}</h3>
                <p className='theme-row-body'>{t(`${base}.danger.deleteBody`, {})}</p>
              </div>
              <div className='theme-row-actions'>
                <Button variant='default' size='sm' onClick={() => setModal('delete')}>
                  {t(`${base}.delete.button`, {})}
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>
    </SettingsSection>
  );
}
