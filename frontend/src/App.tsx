import {
  CSSVariablesResolver,
  MantineProvider,
  type MantineThemeOverride,
  mergeThemeOverrides,
  Text,
} from '@mantine/core';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createBrowserHistory } from 'history';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import AssetReloadNotice from '@/elements/feedback/AssetReloadNotice.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import getAnnouncements from './api/getAnnouncements.ts';
import getLanguages from './api/getLanguages.ts';
import getSettings from './api/getSettings.ts';
import ErrorBoundary from './elements/ErrorBoundary.tsx';
import ExtensionSlot from './elements/ExtensionSlot.tsx';
import { accentFor } from './lib/accent.ts';
import { seasonalAccentColor, seasonFor } from './lib/seasonalTheme.ts'; // plugged.host
import { CurrentWindowProvider } from './providers/CurrentWindowProvider.tsx';
import { HistoryContext } from './providers/contexts/historyContext.ts';
import { ToastProvider } from './providers/ToastProvider.tsx';
import TranslationProvider from './providers/TranslationProvider.tsx';
import { WindowProvider } from './providers/WindowProvider.tsx';
import RouterRoutes from './RouterRoutes.tsx';
import TransitionHistoryRouter from './routers/TransitionHistoryRouter.tsx';
import { useGlobalStore } from './stores/global.ts';

import '@mantine/core/styles.css';
import '@gfazioli/mantine-window/styles.css';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // plugged.host: a page that was just loaded is not reloaded again on every navigation.
      // Pages that must be live set their own refetchInterval.
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      refetchOnMount: true,
      refetchOnReconnect: true,
      retry: 1,
    },
  },
});

const browserHistory = createBrowserHistory();

const MAX_RETRIES = 10;

export default function App({
  theme,
  cssVariablesResolver,
}: {
  theme: MantineThemeOverride;
  cssVariablesResolver: CSSVariablesResolver | null;
}) {
  const { settings, setSettings, setLanguages, setTimeOffset, setAnnouncements } = useGlobalStore(
    useShallow((state) => ({
      settings: state.settings,
      setSettings: state.setSettings,
      setLanguages: state.setLanguages,
      setTimeOffset: state.setTimeOffset,
      setAnnouncements: state.setAnnouncements,
    })),
  );
  const [loadWarning, setLoadWarning] = useState(false);
  const retryCount = useRef(0);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;

    const loadData = () => {
      const loadSettings =
        Object.keys(useGlobalStore.getState().settings).length > 0 ? Promise.resolve(null) : getSettings();
      const loadLanguages = useGlobalStore.getState().languages.length > 0 ? Promise.resolve(null) : getLanguages();

      Promise.all([loadSettings, loadLanguages])
        .then(([settings, languages]) => {
          if (cancelled) return;

          if (settings) {
            setSettings(settings);
            setTimeOffset(Date.now() - new Date(settings.time).getTime());
          }
          if (languages) setLanguages(languages);
        })
        .catch((err) => {
          if (cancelled) return;
          retryCount.current++;
          console.error(`Failed to load app data (attempt ${retryCount.current}):`, err);

          if (retryCount.current >= 2) {
            setLoadWarning(true);
          }

          if (retryCount.current < MAX_RETRIES) {
            timer = setTimeout(loadData, Math.min(retryCount.current * 2000, 10000));
          }
        });

      getAnnouncements()
        .then((announcements) => {
          setAnnouncements(announcements);
        })
        .catch((err) => {
          console.error('Failed to load announcements:', err);
        });
    };

    loadData();

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    if (settings?.app?.icon) {
      const icons = document.getElementsByClassName('app-icon');

      for (const icon of icons) {
        (icon as HTMLLinkElement).href = settings.app.icon;
      }
    }
  }, [settings?.app?.icon]);

  // plugged.host: Theme v2's accent from the theme colour: Mantine's blue palette and the
  // --theme-accent* tokens (lib/accent.ts). Theme v2's own blue changes nothing. A seasonal theme
  // brings its own accent, tints the surfaces and Mantine's dark ramp (lib/seasonalTheme.ts) and
  // marks the root for its decorations.
  const seasonalTheme = settings?.app?.seasonalTheme;
  const season = useMemo(() => seasonFor(seasonalTheme), [seasonalTheme]);
  const accent = useMemo(
    () => accentFor(seasonalAccentColor(seasonalTheme, settings?.app?.themeColor)),
    [seasonalTheme, settings?.app?.themeColor],
  );
  const accentedTheme = useMemo(
    () =>
      accent || season
        ? mergeThemeOverrides(theme, {
            colors: { ...(accent ? { blue: accent.palette } : {}), ...(season ? { dark: season.dark } : {}) },
          })
        : theme,
    [theme, accent, season],
  );

  useLayoutEffect(() => {
    const tokens = Object.entries({ ...season?.tokens, ...accent?.tokens });

    if (season && seasonalTheme) {
      document.documentElement.dataset.season = seasonalTheme;
    }

    for (const [name, value] of tokens) {
      document.documentElement.style.setProperty(name, value);
    }

    return () => {
      for (const [name] of tokens) {
        document.documentElement.style.removeProperty(name);
      }
      delete document.documentElement.dataset.season;
    };
  }, [accent, season, seasonalTheme]);

  return (
    <>
      {Object.keys(settings).length > 0 ? (
        <ErrorBoundary theme={accentedTheme}>
          <MantineProvider
            theme={accentedTheme}
            cssVariablesResolver={cssVariablesResolver || undefined}
            defaultColorScheme='dark'
            forceColorScheme='dark'
            deduplicateInlineStyles
          >
            <QueryClientProvider client={queryClient}>
              <TranslationProvider>
                <ToastProvider>
                  <WindowProvider>
                    <CurrentWindowProvider id={null}>
                      <HistoryContext.Provider value={browserHistory}>
                        <TransitionHistoryRouter history={browserHistory}>
                          <ExtensionSlot
                            components={window.extensionContext.extensionRegistry.global.prependedComponents}
                            name='global-prepended'
                          />

                          <RouterRoutes isNormal />

                          <ExtensionSlot
                            components={window.extensionContext.extensionRegistry.global.appendedComponents}
                            name='global-appended'
                          />
                        </TransitionHistoryRouter>
                      </HistoryContext.Provider>
                    </CurrentWindowProvider>
                  </WindowProvider>
                </ToastProvider>
              </TranslationProvider>
            </QueryClientProvider>
          </MantineProvider>
        </ErrorBoundary>
      ) : (
        <>
          <MantineProvider
            theme={accentedTheme}
            defaultColorScheme='dark'
            forceColorScheme='dark'
            deduplicateInlineStyles
          >
            <Spinner.Centered />
            {loadWarning && (
              <Text size='sm' c='dimmed' ta='center'>
                Having trouble connecting to the server. Retrying...
              </Text>
            )}
          </MantineProvider>
        </>
      )}

      <AssetReloadNotice />
    </>
  );
}
