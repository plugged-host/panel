// plugged.host: the welcome tour's first part, on the server list, with the Pelican panel's steps
// (docs/features/welcome-tour.md): welcome, the sidebar's rows, the usage card, the account menu,
// then Create Server and the first server, whose console the second part explains, or for a new
// account the empty list and Create Server. A step that cannot apply to this account is left out,
// so "Step 3 of 6" counts what the person will see.
import { useQuery } from '@tanstack/react-query';
import { useLocation, useNavigate } from 'react-router';
import getBalance from '@/api/me/economy/getBalance.ts';
import getServers from '@/api/server/getServers.ts';
import { balanceCardShown } from '@/elements/navigation/SidebarBalance.tsx';
import WelcomeTour, { TourStep } from '@/elements/WelcomeTour.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useWelcomeTour } from '@/plugins/useWelcomeTour.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

const HOME_PATHS = ['/', '/all', '/grouped'];

// the sidebar's rows together (not its logo, usage card or account row), else the whole sidebar
const SIDEBAR_ROWS = ['#sidebar-desktop #sidebar-content > .overflow-y-auto > *', '#sidebar-desktop'];

export default function WelcomeTourPanel() {
  const { t } = useTranslations();
  const location = useLocation();
  const navigate = useNavigate();
  const { tour, update } = useWelcomeTour();
  const app = useGlobalStore((state) => state.settings.app.name);
  const createEnabled = useGlobalStore((state) => !!state.settings.games?.createEnabled);
  const economy = useGlobalStore((state) => state.settings.economy);

  const active = !!tour?.panelActive && HOME_PATHS.includes(location.pathname);
  const { data: servers } = useResource({
    queryKey: [...queryKeys.user.welcomeTour(), 'first-server'],
    queryFn: () => getServers(1),
    enabled: active,
    silent: true,
  });
  // the sidebar's usage card (the same query), for its step
  const balance = useQuery({
    queryKey: queryKeys.user.balance(),
    queryFn: getBalance,
    enabled: active,
  });

  if (!active || !tour || !servers || balance.isPending) {
    return null;
  }

  const first = servers.data[0] ?? null;
  const p = 'elements.welcomeTour.panel';
  const plusName = economy?.plusName || t('pages.account.membership.title', {});

  // a bullet per row the sidebar shows, named as it is there
  const rows: { key: 'home' | 'store' | 'plus' | 'tokens' | 'backups' | 'account'; label: string; shown: boolean }[] = [
    { key: 'home', label: t('pages.account.home.title', {}), shown: true },
    { key: 'store', label: t('pages.account.store.title', {}), shown: !!economy?.creditsEnabled },
    { key: 'plus', label: plusName, shown: !!economy?.membershipEnabled },
    { key: 'tokens', label: t('pages.account.tokens.title', {}), shown: !!economy?.tokensEnabled },
    { key: 'backups', label: t('pages.account.backups.title', {}), shown: true },
    { key: 'account', label: t('pages.account.account.title', {}), shown: true },
  ];

  const steps: TourStep[] = [
    { key: 'welcome', targets: [], heading: t(`${p}.welcome.heading`, { app }), body: t(`${p}.welcome.body`, {}) },
    {
      key: 'navigation',
      targets: SIDEBAR_ROWS,
      together: true,
      heading: t(`${p}.navigation.heading`, {}),
      body: t(`${p}.navigation.body`, {}),
      bullets: rows
        .filter((row) => row.shown)
        .map((row) => t(`${p}.navigation.bullets.${row.key}`, { label: row.label })),
    },
  ];

  if (balance.data && balanceCardShown(balance.data)) {
    steps.push({
      key: 'balance',
      targets: ['#sidebar-desktop .theme-balance'],
      heading: t(`${p}.balance.heading`, {}),
      body: t(`${p}.balance.body`, {}),
      bullets: [
        economy?.creditsEnabled ? t(`${p}.balance.bullets.credits`, {}) : null,
        economy?.tokensEnabled && economy.membershipEnabled
          ? t(`${p}.balance.bullets.tokens`, { plus: `${app} ${plusName}` })
          : null,
        economy?.creditsEnabled && economy.tokensEnabled ? t(`${p}.balance.bullets.click`, {}) : null,
      ].filter((bullet) => bullet !== null),
    });
  }

  steps.push({
    key: 'account',
    targets: ['#sidebar-account-card'],
    heading: t(`${p}.account.heading`, {}),
    body: t(`${p}.account.body`, {}),
  });

  const create: TourStep = {
    key: 'create',
    targets: ['[data-tour="create-server"]'],
    heading: t(`${p}.create.heading`, {}),
    body: t(`${p}.create.body`, {}),
  };
  if (first) {
    // someone with a server: Create Server first, then their server, and the last button walks into it
    if (createEnabled) steps.push(create);
    steps.push({
      key: 'servers',
      targets: ['.theme-server-card'],
      heading: t(`${p}.servers.heading`, {}),
      body: t(`${p}.servers.body`, { name: first.name }),
    });
  } else {
    // a new account: where servers will show up, then the button that makes one
    steps.push({
      key: 'empty',
      targets: ['[data-tour="servers-empty"]', '#dashboard-root .mantine-Paper-root'],
      heading: t(`${p}.empty.heading`, {}),
      body: t(`${p}.empty.body`, {}),
    });
    if (createEnabled) {
      steps.push({
        ...create,
        key: 'createFirst',
        heading: t(`${p}.createFirst.heading`, {}),
        body: t(`${p}.createFirst.body`, {}),
      });
    }
  }

  // this part is done; the second waits for the next console
  const complete = () => update({ action: 'complete' });
  const finish = () => {
    complete();
    if (first) {
      navigate(`/server/${first.uuidShort}`);
    } else if (createEnabled) {
      navigate('/create');
    }
  };
  // "Not now" beside a last button that walks on (into the server, or to Create Server)
  const walksOn = !!first || createEnabled;

  return (
    <WelcomeTour
      steps={steps}
      step={tour.step}
      onStep={(next) => update({ action: 'advance', step: next })}
      acknowledgeLabel={t('elements.welcomeTour.acknowledge', {})}
      finishLabel={
        first
          ? t('elements.welcomeTour.openServer', { name: first.name })
          : createEnabled
            ? t('elements.welcomeTour.createServer', {})
            : t('elements.welcomeTour.finish', {})
      }
      onFinish={finish}
      skipLabel={t('elements.welcomeTour.skip', {})}
      onSkip={() => update({ action: 'skip' })}
      laterLabel={walksOn ? t('elements.welcomeTour.notNow', {}) : undefined}
      onLater={walksOn ? complete : undefined}
    />
  );
}
