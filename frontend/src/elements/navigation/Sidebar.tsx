import {
  faArrowRightFromBracket,
  faBars,
  faCheck,
  faChevronUp,
  faCircleQuestion, // plugged.host
  faEyeSlash,
  faGraduationCap,
  faRotateLeft,
  faStar, // plugged.host
  faUserCog,
  faWindowRestore,
  IconDefinition,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Text, Title } from '@mantine/core';
import classNames from 'classnames';
import { createContext, ReactNode, useContext, useEffect, useMemo, useState } from 'react';
import { MemoryRouter, matchPath, NavLink, useLocation, useNavigate } from 'react-router';
import { makeComponentHookable } from 'shared';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import CloseButton from '@/elements/buttons/CloseButton.tsx';
import Avatar from '@/elements/data-display/Avatar.tsx';
import Card from '@/elements/data-display/Card.tsx';
import SeasonalDecor from '@/elements/navigation/SeasonalDecor.tsx'; // plugged.host
import SidebarGroup from '@/elements/navigation/SidebarGroup.tsx'; // plugged.host
import ContextMenu, { ContextMenuItem } from '@/elements/overlays/ContextMenu.tsx';
import Drawer from '@/elements/overlays/Drawer.tsx';
import ProfileBadge from '@/elements/ProfileBadge.tsx'; // plugged.host
import { useLogoutConfirmation } from '@/elements/useLogoutConfirmation.tsx';
import { isAdmin } from '@/lib/auth/permissions.ts';
import { useCustomIcon } from '@/lib/icons.ts';
import { openUrl } from '@/lib/network/url.ts';
import { isNamedRoutePathAccessible } from '@/lib/routes.ts';
import { resetAllDeviceOverrides, useDeviceOverrideCount } from '@/lib/userSettings.ts';
import { useRedactAddresses } from '@/plugins/privacy/useRedactAddresses.ts';
import { useWelcomeTour } from '@/plugins/useWelcomeTour.ts'; // plugged.host
import { useAuth } from '@/providers/AuthProvider.tsx';
import { usePendingPathname } from '@/providers/contexts/pendingNavigationContext.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useWindows } from '@/providers/WindowProvider.tsx';
import RouterRoutes from '@/RouterRoutes.tsx';
import { preloadRouterForPath } from '@/routers/routerImports.ts';
import { useGlobalStore } from '@/stores/global.ts';
import { useQuickActionsStore } from '@/stores/quickActions.ts';
import { useRelativePageStore } from '@/stores/relativePage.ts';

const CloseMobileMenuContext = createContext<(() => void) | null>(null);
// plugged.host: for links in the footer (SidebarBalance), so they close the mobile menu like the rows
export const useCloseMobileMenu = () => useContext(CloseMobileMenuContext);

type SidebarProps = {
  children: ReactNode;
  header?: ReactNode;
  footer?: ReactNode;
};

function Sidebar({ children, header, footer }: SidebarProps) {
  const { t } = useTranslations();
  const { pathname } = useLocation();

  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const quickActionsOpen = useQuickActionsStore((state) => state.open);
  const pageHeader = useRelativePageStore((state) => state.pageHeader);
  const setHasMobileNavbar = useRelativePageStore((state) => state.setHasMobileNavbar);

  useEffect(() => {
    setHasMobileNavbar(true);
    return () => setHasMobileNavbar(false);
  }, []);

  useEffect(() => {
    setIsMobileMenuOpen(false);
  }, [pathname]);

  useEffect(() => {
    if (quickActionsOpen) setIsMobileMenuOpen(false);
  }, [quickActionsOpen]);

  return (
    <>
      {/* plugged.host: Filament's sidebar toggle without a topbar, a square button in the top
          corner ("p-4"), on the sidebar's colour */}
      <Card
        className='lg:hidden! sticky! top-4 z-50 flex-row! items-center justify-center ms-4 mt-4 h-11 w-11 rounded-xl! bg-(--theme-sidebar)!'
        p={0}
      >
        <ActionIcon
          onClick={() => setIsMobileMenuOpen(true)}
          variant='subtle'
          color='gray'
          size='input-md'
          aria-label={t('elements.sidebar.button.openMenu', {})}
        >
          <FontAwesomeIcon size='lg' icon={faBars} />
        </ActionIcon>
      </Card>

      {/* plugged.host: the seasonal theme's drawing at the end of the bar (SeasonalDecor) */}
      <div className='lg:hidden -mt-11 mb-2 ms-20 me-4 h-11 flex items-center gap-3 min-w-0'>
        <div className='flex flex-col justify-center min-w-0 flex-1'>
          {pageHeader && (
            <>
              <Title order={3} lineClamp={1}>
                {pageHeader.title}
              </Title>
              {pageHeader.subtitle && (
                <Text size='xs' c='dimmed' truncate>
                  {pageHeader.subtitle}
                </Text>
              )}
            </>
          )}
        </div>
        <SeasonalDecor slot='bar' />
      </div>

      <Drawer
        opened={isMobileMenuOpen}
        onClose={() => setIsMobileMenuOpen(false)}
        withCloseButton={false}
        size='100%'
        styles={{ body: { height: '100%' } }}
      >
        <CloseButton size='xl' className='absolute! right-4 z-10' onClick={() => setIsMobileMenuOpen(false)} />

        <CloseMobileMenuContext.Provider value={() => setIsMobileMenuOpen(false)}>
          <div id='sidebar-content' className='h-full flex flex-col'>
            {header && <div className='shrink-0'>{header}</div>}
            <div className='flex flex-col flex-1 overflow-y-auto min-h-0'>{children}</div>
            {footer && (
              <div className='shrink-0 pt-2'>
                {/* plugged.host */}
                <SeasonalDecor slot='shelf' />
                {footer}
              </div>
            )}
          </div>
        </CloseMobileMenuContext.Provider>
      </Drawer>

      <Card
        className='top-0 sticky! hidden! lg:block! h-screen w-64! shrink-0 overflow-hidden transition-[width] duration-200 ease-in-out'
        p='sm'
        radius={0}
        id='sidebar-desktop'
      >
        <div id='sidebar-content' className='h-full flex flex-col'>
          {header && <div className='shrink-0'>{header}</div>}
          <div className='flex flex-col flex-1 overflow-y-auto min-h-0'>{children}</div>
          {footer && (
            <div className='shrink-0 pt-2'>
              {/* plugged.host */}
              <SeasonalDecor slot='shelf' />
              {footer}
            </div>
          )}
        </div>
      </Card>
    </>
  );
}

type LinkProps = {
  to: string;
  end?: boolean;
  icon?: IconDefinition;
  customIcon?: string | null;
  name?: string;
  title?: string;
  className?: string;
  activeMatches?: string[];
};

function Link({ to, end, icon, customIcon, name, title = name, className, activeMatches }: LinkProps) {
  const { t } = useTranslations();
  const { addWindow } = useWindows();
  const closeMobileMenu = useContext(CloseMobileMenuContext);
  const { pathname } = useLocation();
  const pendingPathname = usePendingPathname();
  const resolvedIcon = useCustomIcon(customIcon, icon);
  const extraActive =
    activeMatches?.some((pattern) => matchPath({ path: pattern, end: false }, pendingPathname ?? pathname)) ?? false;

  if (to.endsWith('/*')) to = to.slice(0, to.length - 2);

  const items: ContextMenuItem[] = useMemo(
    () => [
      {
        type: 'action' as const,
        icon: faWindowRestore,
        label: t('elements.sidebar.button.openInVirtualWindow', {}),
        onClick: () => {
          closeMobileMenu?.();
          addWindow(
            title || 'Window',
            <MemoryRouter initialEntries={[to]}>
              <RouterRoutes isNormal={false} />
            </MemoryRouter>,
          );
        },
        color: 'gray',
      },
      {
        type: 'action' as const,
        icon: faWindowRestore,
        label: t('elements.sidebar.button.openInPopup', {}),
        onClick: () =>
          window.open(
            to,
            '_blank',
            'popup=yes,toolbar=no,location=no,status=no,menubar=no,scrollbars=yes,resizable=yes',
          ),
        color: 'gray',
      },
      {
        type: 'action' as const,
        icon: faWindowRestore,
        label: t('elements.sidebar.button.openInNewTab', {}),
        onClick: () => openUrl(to),
        color: 'gray',
      },
    ],
    [t, addWindow, closeMobileMenu, title, to],
  );

  return (
    <ContextMenu items={items}>
      {({ openMenu }) => (
        <NavLink
          to={to}
          end={end}
          onContextMenu={(e) => {
            e.preventDefault();
            openMenu(e.clientX, e.clientY);
          }}
          onPointerEnter={() => preloadRouterForPath(to)}
          onFocus={() => preloadRouterForPath(to)}
          className='w-full'
        >
          {({ isActive: navActive }) => {
            const isActive =
              (pendingPathname !== null ? !!matchPath({ path: to, end: !!end }, pendingPathname) : navActive) ||
              extraActive;

            return (
              <Button
                color={isActive ? 'blue' : 'gray'}
                className={classNames('sidebar-link', isActive && 'cursor-default! active', className)}
                variant='subtle'
                radius={12}
                fullWidth
                styles={{ label: { width: '100%' } }}
              >
                {resolvedIcon && <FontAwesomeIcon icon={resolvedIcon} className='mr-2' />} {name}
              </Button>
            );
          }}
        </NavLink>
      )}
    </ContextMenu>
  );
}

// plugged.host: Filament's sidebar groups. A labelled divider is the group's caption over its
// rows ("fi-sidebar-group-label"), an unlabelled one a hairline between groups.
function Divider({ label }: { label?: string }) {
  return label ? <div className='theme-sidebar-group-label'>{label}</div> : <div className='theme-sidebar-divider' />;
}

// plugged.host: Theme v2's logo block, the panel's logo centred at the top of the sidebar
// ("flex justify-center mt-8", "h-10"), leading home. The banner when one is set, else the icon.
function Logo() {
  const settings = useGlobalStore((state) => state.settings);
  const closeMobileMenu = useContext(CloseMobileMenuContext);

  return (
    <NavLink to='/' className='theme-sidebar-logo' aria-label={settings.app.name} onClick={() => closeMobileMenu?.()}>
      <img src={settings.app.banner ?? settings.app.icon} alt={settings.app.name} />
      {/* plugged.host */}
      <SeasonalDecor slot='logo' />
    </NavLink>
  );
}

function Footer() {
  const { t } = useTranslations();
  const { impersonating, user } = useAuth();
  const navigate = useNavigate();
  const deviceOverrideCount = useDeviceOverrideCount();
  const [redactAddresses, setRedactAddresses] = useRedactAddresses();
  const routeOrder = useGlobalStore((state) => state.settings.user?.routeOrder);

  const { confirmLogout, logoutModal } = useLogoutConfirmation();
  const { enabled: tourEnabled, update: updateTour } = useWelcomeTour(); // plugged.host
  const appName = useGlobalStore((state) => state.settings.app.name); // plugged.host
  const reviews = useGlobalStore((state) => state.settings.reviews); // plugged.host

  const accountRouteDisabled = !isNamedRoutePathAccessible(routeOrder, '/');

  const suspended = Boolean(user?.suspended);
  const accountHidden = suspended || accountRouteDisabled;
  const adminHidden = suspended || !user || !isAdmin(user);

  const items: ContextMenuItem[] = useMemo(
    () => [
      {
        type: 'action' as const,
        icon: faUserCog,
        label: t('pages.account.account.title', {}),
        hidden: accountHidden,
        onClick: () => navigate('/account'),
      },
      {
        type: 'divider' as const,
        hidden: accountHidden || adminHidden,
      },
      {
        type: 'action' as const,
        icon: faGraduationCap,
        label: t('pages.account.admin.title', {}),
        hidden: adminHidden,
        onClick: () => navigate('/admin'),
      },
      {
        type: 'divider' as const,
        hidden: accountHidden && adminHidden,
      },
      {
        type: 'action' as const,
        icon: faEyeSlash,
        label: t('elements.sidebar.button.redactAddresses', {}),
        rightSection: redactAddresses && <FontAwesomeIcon icon={faCheck} size='sm' />,
        onClick: () => setRedactAddresses(!redactAddresses),
      },
      {
        type: 'action' as const,
        icon: faRotateLeft,
        label: t('elements.sidebar.button.resetDeviceOverrides', { count: deviceOverrideCount }),
        hidden: deviceOverrideCount === 0,
        onClick: () => resetAllDeviceOverrides(),
      },
      // plugged.host: the welcome tour again, from the server list (docs/features/welcome-tour.md)
      {
        type: 'action' as const,
        icon: faCircleQuestion,
        label: t('elements.welcomeTour.replay', {}),
        hidden: !tourEnabled || suspended,
        onClick: () => {
          updateTour({ action: 'restart' });
          navigate('/');
        },
      },
      // plugged.host: the review site, for everyone (docs/features/reviews.md)
      {
        type: 'action' as const,
        icon: faStar,
        label: t('elements.sidebar.button.review', { app: appName, site: reviews?.site ?? '' }),
        hidden: !reviews?.url || suspended,
        onClick: () => openUrl(reviews.url),
      },
      {
        type: 'divider' as const,
      },
      {
        type: 'action' as const,
        icon: faArrowRightFromBracket,
        label: impersonating
          ? t('elements.sidebar.button.stopImpersonating', {})
          : t('elements.sidebar.button.logout', {}),
        color: 'red',
        onClick: confirmLogout,
      },
    ],
    [
      t,
      navigate,
      confirmLogout,
      accountHidden,
      adminHidden,
      deviceOverrideCount,
      redactAddresses,
      setRedactAddresses,
      impersonating,
      tourEnabled, // plugged.host
      updateTour, // plugged.host
      appName, // plugged.host
      reviews, // plugged.host
      suspended,
    ],
  );

  if (!user) {
    return null;
  }

  const displayName = user.nameFirst && user.nameLast ? `${user.nameFirst} ${user.nameLast}` : user.username;

  // plugged.host: Theme v2's avatar row at the foot of the sidebar, Filament's user menu: the
  // whole row opens the menu (account, admin, sign out) above it.
  return (
    <>
      <ContextMenu items={items} menuProps={{ position: 'top-start', offset: 8 }}>
        {({ openMenu }) => (
          <button
            type='button'
            className='theme-sidebar-user'
            id='sidebar-account-card'
            aria-haspopup='menu'
            onClick={(e) => {
              const rect = e.currentTarget.getBoundingClientRect();
              openMenu(rect.left, rect.top);
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(e.clientX, e.clientY);
            }}
          >
            <Avatar size={32} className='select-none shrink-0' src={user.avatar} name={user.username} />
            <span className='theme-sidebar-user-name'>{displayName}</span>
            <ProfileBadge badge={user.badge} className='shrink-0' /> {/* plugged.host */}
            <FontAwesomeIcon icon={faChevronUp} className='theme-sidebar-user-chevron' />
          </button>
        )}
      </ContextMenu>

      {logoutModal}
    </>
  );
}

export default makeComponentHookable(Sidebar, {
  Link: makeComponentHookable(Link),
  Divider: makeComponentHookable(Divider),
  Group: makeComponentHookable(SidebarGroup), // plugged.host
  Logo: makeComponentHookable(Logo),
  Footer: makeComponentHookable(Footer),
});
