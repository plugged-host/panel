// plugged.host: the settings page's jump bar, the Pelican panel's ".phsf-nav" (server-form.blade.php):
// in-page links to the page's sections in its order, Danger Zone red at the end, the section in view
// marked while the page scrolls. The section the address names (/settings#sftp, also where the old
// /settings/<section> addresses land) is brought into view, and kept there while the sections above
// it still load (docs/features/server-settings.md)
import classNames from 'classnames';
import { useEffect, useRef, useState } from 'react';
import { Link, useLocation } from 'react-router';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import {
  SettingsSection,
  useBehaviourShown,
  useCanDeleteServer,
  useResizeOptions,
  useSoftwareOptions,
} from './useSettingsSections.ts';

// how long a jump keeps its section in view while the page above it still grows
const FOLLOW_MS = 3000;
// what the reader does to take the page back from that
const READER_INPUT = ['wheel', 'touchstart', 'keydown', 'mousedown'] as const;
// as Pelican: the first section near the top of the page, the last at its end
const TOP_PX = 120;
const END_PX = 60;
// a section counts as in view once its top is this far under the bar (the gap scrolling to it leaves)
const UNDER_BAR_PX = 32;

export default function ServerSettingsNav() {
  const { t } = useTranslations();
  const location = useLocation();
  const navRef = useRef<HTMLElement>(null);

  const canSftp = useServerCan('files.sftp');
  const canBehaviour = useBehaviourShown();
  const canNotifications = useServerCan('backups.read');
  const canReinstall = useServerCan('settings.install');
  const canDelete = useCanDeleteServer();
  const resize = useResizeOptions();
  const software = useSoftwareOptions();

  const items: { id: SettingsSection; label: string; shown: boolean; danger?: boolean }[] = [
    { id: 'general', label: t('pages.server.settings.nav.general', {}), shown: true },
    { id: 'sftp', label: t('pages.server.settings.nav.sftp', {}), shown: canSftp },
    { id: 'resources', label: t('pages.server.settings.nav.resources', {}), shown: resize.usable },
    { id: 'version', label: t('pages.server.settings.nav.version', {}), shown: software.usable },
    { id: 'behaviour', label: t('pages.server.settings.nav.behaviour', {}), shown: canBehaviour },
    { id: 'notifications', label: t('pages.server.settings.nav.notifications', {}), shown: canNotifications },
    { id: 'danger', label: t('pages.server.settings.nav.danger', {}), shown: canReinstall || canDelete, danger: true },
  ];
  const shown = items.filter((item) => item.shown);
  const ids = shown.map((item) => item.id);
  const idsKey = ids.join(' ');

  const [active, setActive] = useState<string>('general');
  // a section of this page: looked up in its own column, as a second copy of the page (in a window)
  // has the same ids
  const sectionElement = (id: string) => navRef.current?.parentElement?.querySelector<HTMLElement>(`#${id}`) ?? null;
  // the section a jump is keeping in view; it stays marked until the reader scrolls
  const following = useRef<string | null>(null);

  // the section in view, as Pelican's onScroll: the last one whose top went under the bar
  useEffect(() => {
    const update = () => {
      if (following.current) {
        setActive(following.current);
        return;
      }

      const root = document.documentElement;
      const scrollY = window.scrollY || root.scrollTop;
      if (scrollY < TOP_PX) {
        setActive(ids[0]);
        return;
      }
      if (window.innerHeight + scrollY >= root.scrollHeight - END_PX) {
        setActive(ids[ids.length - 1]);
        return;
      }

      const line = (navRef.current?.getBoundingClientRect().bottom ?? 0) + UNDER_BAR_PX;
      let current = ids[0];
      for (const id of ids) {
        const element = sectionElement(id);
        if (element && element.getBoundingClientRect().top <= line) {
          current = id;
        }
      }
      setActive(current);
    };

    update();
    window.addEventListener('scroll', update, { passive: true });
    window.addEventListener('resize', update);

    return () => {
      window.removeEventListener('scroll', update);
      window.removeEventListener('resize', update);
    };
  }, [idsKey]);

  // the section the address names: into view now and again whenever the page grows (a section above
  // it finished loading), until the reader scrolls or a few seconds have passed. The page mounts this
  // bar once its sections are known, so the list it checks against is the final one.
  useEffect(() => {
    const id = location.hash.slice(1);
    if (!ids.includes(id as SettingsSection)) return;

    following.current = id;
    setActive(id);

    const bring = () => sectionElement(id)?.scrollIntoView({ block: 'start' });
    const column = navRef.current?.parentElement;
    // a ResizeObserver calls back once right away, which brings it into view the first time
    const observer = new ResizeObserver(bring);
    if (column) {
      observer.observe(column);
    } else {
      bring();
    }

    const stop = () => {
      following.current = null;
      observer.disconnect();
    };
    const timer = window.setTimeout(stop, FOLLOW_MS);
    for (const event of READER_INPUT) {
      window.addEventListener(event, stop, { passive: true });
    }

    return () => {
      window.clearTimeout(timer);
      for (const event of READER_INPUT) {
        window.removeEventListener(event, stop);
      }
      stop();
    };
  }, [location.hash, location.key]);

  return (
    <nav ref={navRef} className='theme-jump' aria-label={t('pages.server.settings.nav.label', {})}>
      {shown.map((item) => (
        <Link
          key={item.id}
          to={{ pathname: location.pathname, search: location.search, hash: `#${item.id}` }}
          replace
          aria-current={active === item.id ? 'location' : undefined}
          className={classNames(
            'theme-jump-item',
            item.danger && 'theme-jump-item-danger',
            active === item.id && 'active',
          )}
        >
          {item.label}
        </Link>
      ))}
    </nav>
  );
}
