import { faArrowUpRightFromSquare, faCircleUser, faGraduationCap, faServer } from '@fortawesome/free-solid-svg-icons';
import { Suspense, useMemo } from 'react';
import { Navigate, Route, Routes } from 'react-router'; // plugged.host: Navigate
import { AdLayoutFoot, AdLayoutTop } from '@/elements/ads/AdLayout.tsx'; // plugged.host
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import ScreenBlock from '@/elements/feedback/ScreenBlock.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Container from '@/elements/layout/Container.tsx';
import ServerSwitcher from '@/elements/navigation/ServerSwitcher.tsx';
import Sidebar from '@/elements/navigation/Sidebar.tsx';
import SidebarBalance from '@/elements/navigation/SidebarBalance.tsx'; // plugged.host
import SidebarNotifications from '@/elements/navigation/SidebarNotifications.tsx'; // plugged.host
import QuickActionsTrigger from '@/elements/quickActions/QuickActionsTrigger.tsx';
import { isAdmin } from '@/lib/auth/permissions.ts';
import { resolveString } from '@/lib/lazy.ts';
import { getAccessibleRoutePaths, to } from '@/lib/routes.ts';
import CreateServer from '@/pages/dashboard/create/CreateServer.tsx'; // plugged.host
import DashboardHomeAll from '@/pages/dashboard/home/DashboardHomeAll.tsx';
import DashboardHomeGrouped from '@/pages/dashboard/home/DashboardHomeGrouped.tsx';
import ReviewPrompt from '@/pages/dashboard/home/ReviewPrompt.tsx'; // plugged.host
import WelcomeTourPanel from '@/pages/dashboard/home/WelcomeTourPanel.tsx'; // plugged.host
import { useStartOnGroupedServers } from '@/plugins/server/useStartOnGroupedServers.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import accountRoutes from '@/routers/routes/accountRoutes.ts';
import { legacyDashboardRedirects } from '@/routers/routes/legacyRoutes.ts'; // plugged.host
import { useGlobalStore } from '@/stores/global.ts';

// plugged.host: the account pages the Pelican panel's sidebar shows as rows of their own, in its
// order (Resource Store, Plus, Tokens, Backups); the others fold under Account.
const MAIN_PATHS = ['/store', '/membership', '/tokens', '/backups'];

export default function DashboardRouter({ isNormal }: { isNormal: boolean }) {
  const { t, language } = useTranslations();
  const { user } = useAuth();
  const [startOnGroupedServers] = useStartOnGroupedServers();
  const routeOrder = useGlobalStore((state) => state.settings.user?.routeOrder);

  const allAccountRoutes = useMemo(() => {
    const routes = [...accountRoutes, ...window.extensionContext.extensionRegistry.routes.accountRoutes];

    for (const interceptor of window.extensionContext.extensionRegistry.routes.accountRouteInterceptors) {
      interceptor(routes);
    }

    return routes;
  }, []);

  const sidebarItems = useMemo(() => {
    if (!routeOrder) {
      return allAccountRoutes
        .filter((route) => !!route.name && (!route.filter || route.filter()))
        .map((route) => ({ type: 'route' as const, route, icon: null }));
    }

    return routeOrder
      .map((item) => {
        if (item.type === 'route') {
          const route = allAccountRoutes.find((r) => r.path === item.path);
          if (!route || !route.name || (route.filter && !route.filter())) return null;
          return { type: 'route' as const, route, icon: item.icon };
        }

        if (item.type === 'divider') {
          const label = (language !== 'en' && item.nameTranslations[language]) || item.name || undefined;
          return { type: 'divider' as const, label };
        }

        if (item.type === 'redirect') {
          const name = (language !== 'en' && item.nameTranslations[language]) || item.name;
          return { type: 'redirect' as const, name, destination: item.destination, icon: item.icon };
        }

        return null;
      })
      .filter(Boolean);
  }, [routeOrder, allAccountRoutes, language]);

  const accessibleRoutePaths = useMemo(
    () => getAccessibleRoutePaths(allAccountRoutes, routeOrder),
    [allAccountRoutes, routeOrder],
  );

  // plugged.host: the Pelican panel's rows at the top, in its order; the account's own pages fold
  // under one Account row; Admin last.
  const mainRoutes = useMemo(
    () =>
      MAIN_PATHS.map((path) => allAccountRoutes.find((route) => route.path === path)).filter(
        (route) =>
          !!route &&
          !!route.name &&
          (!route.filter || route.filter()) &&
          (!accessibleRoutePaths || accessibleRoutePaths.has(route.path)),
      ),
    [allAccountRoutes, accessibleRoutePaths],
  );
  const accountItems = useMemo(
    () => sidebarItems.filter((item) => item?.type !== 'route' || !MAIN_PATHS.includes(item.route.path)),
    [sidebarItems],
  );

  return (
    <div className='lg:flex h-full'>
      {isNormal && (
        <Sidebar
          header={
            <>
              <Sidebar.Logo />
              {!user?.suspended && (
                <>
                  <QuickActionsTrigger />
                  {/* plugged.host: the notification inbox (docs/features/notifications.md) */}
                  <SidebarNotifications />
                  <div className='mt-2' />
                </>
              )}
            </>
          }
          footer={
            <>
              {/* plugged.host: the usage card and balances */}
              {!user?.suspended && <SidebarBalance />}
              {!user?.suspended && <ServerSwitcher className='mb-2' />}
              <Sidebar.Footer />
            </>
          }
        >
          {!user?.suspended && (
            <>
              <Sidebar.Link
                to='/'
                end
                icon={faServer}
                name={t('pages.account.home.title', {})}
                activeMatches={['/grouped', '/create']}
              />
              {mainRoutes.map(
                (route) =>
                  route && (
                    <Sidebar.Link
                      key={route.path}
                      to={to(route.path, '/account')}
                      end={route.exact}
                      icon={route.icon}
                      name={resolveString(route.name)}
                      activeMatches={route.activeMatches}
                    />
                  ),
              )}

              <Sidebar.Group
                id='account'
                icon={faCircleUser}
                name={t('pages.account.account.title', {})}
                activeMatches={accountItems.flatMap((item) =>
                  item?.type === 'route'
                    ? [{ path: to(item.route.path, '/account'), end: item.route.path === '/' }]
                    : [],
                )}
              >
                {accountItems.map((item, index) => {
                  if (!item) return null;

                  if (item.type === 'divider') {
                    return <Sidebar.Divider key={`divider-${index}`} label={item.label} />;
                  }

                  if (item.type === 'redirect') {
                    return (
                      <Sidebar.Link
                        key={`redirect-${index}`}
                        to={item.destination}
                        icon={faArrowUpRightFromSquare}
                        customIcon={item.icon}
                        name={item.name}
                      />
                    );
                  }

                  const { route } = item;

                  return (
                    <Sidebar.Link
                      key={route.path}
                      to={to(route.path, '/account')}
                      end={route.exact}
                      icon={route.icon}
                      customIcon={item.icon}
                      name={route.path === '/' ? t('elements.sidebar.profileSettings', {}) : resolveString(route.name)}
                      activeMatches={route.activeMatches}
                    />
                  );
                })}
              </Sidebar.Group>

              {isAdmin(user) && (
                <Sidebar.Link to='/admin' end icon={faGraduationCap} name={t('pages.account.admin.title', {})} />
              )}
            </>
          )}
        </Sidebar>
      )}

      <div
        id='dashboard-root'
        className={isNormal ? 'max-w-[100vw] min-w-0 flex-1 lg:ml-0' : 'flex-1 lg:ml-0 overflow-auto h-full'}
      >
        <Container isNormal={isNormal}>
          {user?.suspended ? (
            <ScreenBlock
              title={t('elements.screenBlock.suspended.title', {})}
              content={t('elements.screenBlock.suspended.content', {})}
            />
          ) : (
            <>
              <ExtensionSlot
                components={window.extensionContext.extensionRegistry.pages.dashboard.prependedComponents}
                name='dashboard-prepended-component'
              />
              <AdLayoutTop area='account' /> {/* plugged.host: docs/features/ads.md */}
              <ReviewPrompt /> {/* plugged.host: on the server list, docs/features/reviews.md */}
              <Suspense fallback={<Spinner.Centered />}>
                <Routes>
                  {startOnGroupedServers ? (
                    <>
                      <Route path='' element={<DashboardHomeGrouped />} />
                      <Route path='/all' element={<DashboardHomeAll />} />
                    </>
                  ) : (
                    <>
                      <Route path='' element={<DashboardHomeAll />} />
                      <Route path='/grouped' element={<DashboardHomeGrouped />} />
                    </>
                  )}
                  <Route path='/create' element={<CreateServer />} /> {/* plugged.host */}
                  {allAccountRoutes
                    .filter((route) => !route.filter || route.filter())
                    .filter((route) => !accessibleRoutePaths || accessibleRoutePaths.has(route.path))
                    .map(({ path, element: Element }) => (
                      <Route key={path} path={`/account/${path}`.replace('//', '/')} element={<Element />} />
                    ))}
                  {/* plugged.host: the Pelican panel's page paths (legacyRoutes.ts) */}
                  {Object.entries(legacyDashboardRedirects).map(([from, target]) => (
                    <Route key={from} path={from} element={<Navigate to={target} replace />} />
                  ))}
                  <Route
                    path='*'
                    element={
                      <AccountContentContainer title={t('elements.screenBlock.notFound.title', {})}>
                        <ScreenBlock
                          title={t('elements.screenBlock.notFound.title', {})}
                          content={t('elements.screenBlock.notFound.content', {})}
                        />
                      </AccountContentContainer>
                    }
                  />
                </Routes>
              </Suspense>
              <AdLayoutFoot area='account' /> {/* plugged.host */}
              <WelcomeTourPanel /> {/* plugged.host: on the server list */}
              <ExtensionSlot
                components={window.extensionContext.extensionRegistry.pages.dashboard.appendedComponents}
                name='dashboard-appended-component'
              />
            </>
          )}
        </Container>
      </div>
    </div>
  );
}
