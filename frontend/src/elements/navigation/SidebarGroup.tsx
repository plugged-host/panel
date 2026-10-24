// plugged.host: a sidebar row that folds a group of rows away (the account's pages), Filament's
// collapsible sidebar group drawn as one of the rows: icon, label, an arrow at the end. It opens by
// itself on one of its pages and otherwise stays the way the person left it.
import { faChevronDown, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import classNames from 'classnames';
import { ReactNode, useEffect, useId, useState } from 'react';
import { matchPath, useLocation } from 'react-router';
import Button from '@/elements/buttons/Button.tsx';
import Collapse from '@/elements/layout/Collapse.tsx';
import { usePendingPathname } from '@/providers/contexts/pendingNavigationContext.ts';

type Props = {
  id: string;
  icon?: IconDefinition;
  name: string;
  /** The group's pages (`end`: that path only, not the pages under it): it opens on them. */
  activeMatches?: { path: string; end: boolean }[];
  children: ReactNode;
};

const storageKey = (id: string) => `sidebar-group:${id}`;

function storedOpen(id: string): boolean | null {
  try {
    const value = window.localStorage.getItem(storageKey(id));
    return value === null ? null : value === '1';
  } catch {
    return null;
  }
}

export default function SidebarGroup({ id, icon, name, activeMatches, children }: Props) {
  const { pathname } = useLocation();
  const pendingPathname = usePendingPathname();
  const contentId = useId();
  const inside = activeMatches?.some((match) => matchPath(match, pendingPathname ?? pathname)) ?? false;
  const [open, setOpen] = useState(() => inside || (storedOpen(id) ?? false));

  useEffect(() => {
    if (inside) setOpen(true);
  }, [inside]);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    try {
      window.localStorage.setItem(storageKey(id), next ? '1' : '0');
    } catch {
      // nothing is kept; the group still opens and closes
    }
  };

  return (
    <div className='theme-sidebar-group'>
      <Button
        color='gray'
        className={classNames('sidebar-link', 'theme-sidebar-group-toggle', open && 'open')}
        variant='subtle'
        radius={12}
        fullWidth
        styles={{ label: { width: '100%' } }}
        aria-expanded={open}
        aria-controls={contentId}
        onClick={toggle}
      >
        {icon && <FontAwesomeIcon icon={icon} className='mr-2' />}
        <span className='theme-sidebar-group-name'>{name}</span>
        <FontAwesomeIcon icon={faChevronDown} className='theme-sidebar-group-arrow' />
      </Button>
      <Collapse expanded={open} id={contentId}>
        <div className='theme-sidebar-group-items'>{children}</div>
      </Collapse>
    </div>
  );
}
