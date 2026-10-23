import { lazy, Suspense, useEffect, useMemo } from 'react';
import { Route, Routes } from 'react-router';
import ScreenBlock from '@/elements/feedback/ScreenBlock.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import { ContextMenuProvider } from '@/elements/overlays/ContextMenu.tsx';
import OobeGuard from '@/routers/guards/OobeGuard.tsx';
import ContentContainer from './elements/containers/ContentContainer.tsx';
import ExtensionSlot from './elements/ExtensionSlot.tsx';
import UploadConflictHost from './elements/files/UploadConflictHost.tsx';
import UploadsCard from './elements/files/UploadsCard.tsx';
import QuickActionsPalette from './elements/quickActions/QuickActionsPalette.tsx';
import { isAdmin } from './lib/auth/permissions.ts';
import { AuthProvider, useAuth } from './providers/AuthProvider.tsx';
import { useCurrentWindow } from './providers/CurrentWindowProvider.tsx';
import { useTranslations } from './providers/TranslationProvider.tsx';
import { useWindows } from './providers/WindowProvider.tsx';
import AdminGuard from './routers/guards/AdminGuard.tsx';
import AuthenticatedGuard from './routers/guards/AuthenticatedGuard.tsx';
import UnauthenticatedGuard from './routers/guards/UnauthenticatedGuard.tsx';
import { prefetchRouters, routerImports } from './routers/routerImports.ts';
import globalRoutes from './routers/routes/globalRoutes.ts';
import { AdminStoreContextProvider, createAdminStore } from './stores/admin.tsx';
import {
  createRelativePageStore,
  RelativePageStoreContextProvider,
  useRelativePageStore,
} from './stores/relativePage.ts';
import { createServerStore, ServerStoreContextProvider } from './stores/server.ts';

const OobeRouter = lazy(routerImports.oobe);
const AuthenticationRouter = lazy(routerImports.authentication);
const DashboardRouter = lazy(routerImports.dashboard);
const AdminRouter = lazy(routerImports.admin);
const ServerRouter = lazy(routerImports.server);

function RelativePageListener() {
  const { updateWindow } = useWindows();
  const title = useRelativePageStore((state) => state.title);
  const { id } = useCurrentWindow();

  useEffect(() => {
    if (id) {
      updateWindow(id, title);
    } else {
      document.title = title;
    }
  }, [id, title]);

  return null;
}

function RouterPrefetcher() {
  const { user } = useAuth();
  const userUuid = user?.uuid;
  const admin = isAdmin(user);

  useEffect(() => {
    if (userUuid) prefetchRouters(admin);
  }, [userUuid, admin]);

  return null;
}

export default function RouterRoutes({ isNormal }: { isNormal: boolean }) {
  const { t } = useTranslations();

  const allGlobalRoutes = useMemo(() => {
    const routes = [...globalRoutes, ...window.extensionContext.extensionRegistry.routes.globalRoutes];

    for (const interceptor of window.extensionContext.extensionRegistry.routes.globalRouteInterceptors) {
      interceptor(routes);
    }

    return routes;
  }, []);

  return (
    <ContextMenuProvider>
      <RelativePageStoreContextProvider createStore={createRelativePageStore}>
        <AdminStoreContextProvider createStore={createAdminStore}>
          <ServerStoreContextProvider createStore={createServerStore}>
            <AuthProvider>
              <ExtensionSlot
                components={window.extensionContext.extensionRegistry.pages.global.prependedComponents}
                name='pagesGlobal-prepended'
              />

              <Suspense fallback={<Spinner.Centered />}>
                <Routes>
                  {allGlobalRoutes
                    .filter((route) => !route.filter || route.filter())
                    .map(({ path, element: Element }) => (
                      <Route key={path} path={path} element={<Element />} />
                    ))}

                  <Route element={<OobeGuard />}>
                    <Route path='/oobe/*' element={<OobeRouter />} />

                    <Route element={<UnauthenticatedGuard />}>
                      <Route path='/auth/*' element={<AuthenticationRouter />} />
                    </Route>

                    <Route element={<AuthenticatedGuard />}>
                      <Route path='/server/:id/*' element={<ServerRouter isNormal={isNormal} />} />
                      <Route path='/*' element={<DashboardRouter isNormal={isNormal} />} />

                      <Route element={<AdminGuard />}>
                        <Route path='/admin/*' element={<AdminRouter isNormal={isNormal} />} />
                      </Route>
                    </Route>

                    <Route
                      path='*'
                      element={
                        <ContentContainer title={t('elements.screenBlock.notFound.title', {})}>
                          <ScreenBlock
                            title={t('elements.screenBlock.notFound.title', {})}
                            content={t('elements.screenBlock.notFound.content', {})}
                          />
                        </ContentContainer>
                      }
                    />
                  </Route>
                </Routes>
              </Suspense>

              {isNormal && <UploadsCard />}
              {isNormal && <UploadConflictHost />}
              {isNormal && <QuickActionsPalette />}

              <ExtensionSlot
                components={window.extensionContext.extensionRegistry.pages.global.appendedComponents}
                name='pagesGlobal-appended'
              />
              <RelativePageListener />
              {isNormal && <RouterPrefetcher />}
            </AuthProvider>
          </ServerStoreContextProvider>
        </AdminStoreContextProvider>
      </RelativePageStoreContextProvider>
    </ContextMenuProvider>
  );
}
