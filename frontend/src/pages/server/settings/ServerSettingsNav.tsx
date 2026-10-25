// plugged.host: the settings pages' jump bar, the Pelican panel's ".phsf-nav" on its settings page
// (server-form.blade.php): text items in its order, Danger Zone red at the end
// (docs/features/server-settings.md)
import classNames from 'classnames';
import { NavLink, useParams } from 'react-router';
import { to } from '@/lib/routes.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import {
  BEHAVIOUR_PERMISSIONS,
  useCanDeleteServer,
  useResizeOptions,
  useSoftwareOptions,
} from './useSettingsSections.ts';

export default function ServerSettingsNav() {
  const { t } = useTranslations();
  const params = useParams<'id'>();
  const devboxesEnabled = useGlobalStore((state) => !!state.settings.games?.devboxesEnabled);

  const canSftp = useServerCan('files.sftp');
  const canDevbox = useServerCan('devbox.connect');
  const canStartup = useServerCan('startup.read');
  const canBehaviour = useServerCan(BEHAVIOUR_PERMISSIONS);
  const canNotifications = useServerCan('backups.read');
  const canReinstall = useServerCan('settings.install');
  const canDelete = useCanDeleteServer();
  const resize = useResizeOptions();
  const software = useSoftwareOptions();

  const base = `/server/${params.id}`;
  const items = [
    { path: '/settings', label: t('pages.server.settings.nav.general', {}), shown: true },
    { path: '/settings/sftp', label: t('pages.server.settings.nav.sftp', {}), shown: canSftp },
    { path: '/devbox', label: t('pages.server.settings.nav.vscode', {}), shown: devboxesEnabled && canDevbox },
    { path: '/settings/resources', label: t('pages.server.settings.nav.resources', {}), shown: resize.usable },
    { path: '/startup', label: t('pages.server.settings.nav.startup', {}), shown: canStartup },
    { path: '/settings/version', label: t('pages.server.settings.nav.version', {}), shown: software.usable },
    { path: '/settings/behaviour', label: t('pages.server.settings.nav.behaviour', {}), shown: canBehaviour },
    {
      path: '/settings/notifications',
      label: t('pages.server.settings.nav.notifications', {}),
      shown: canNotifications,
    },
    {
      path: '/settings/danger',
      label: t('pages.server.settings.nav.danger', {}),
      shown: canReinstall || canDelete,
      danger: true,
    },
  ];

  return (
    <nav className='theme-jump' aria-label={t('pages.server.settings.nav.label', {})}>
      {items
        .filter((item) => item.shown)
        .map((item) => (
          <NavLink
            key={item.path}
            to={to(item.path, base)}
            end
            className={classNames('theme-jump-item', item.danger && 'theme-jump-item-danger')}
          >
            {item.label}
          </NavLink>
        ))}
    </nav>
  );
}
