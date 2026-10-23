// plugged.host: the welcome tour's first part, on the server list (docs/features/welcome-tour.md):
// the sidebar, the account menu, Create Server and the first server, whose console the second
// part then explains.
import { useLocation, useNavigate } from 'react-router';
import getServers from '@/api/server/getServers.ts';
import WelcomeTour, { TourStep } from '@/elements/WelcomeTour.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useWelcomeTour } from '@/plugins/useWelcomeTour.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

const HOME_PATHS = ['/', '/all', '/grouped'];

export default function WelcomeTourPanel() {
  const { t } = useTranslations();
  const location = useLocation();
  const navigate = useNavigate();
  const { tour, update } = useWelcomeTour();
  const app = useGlobalStore((state) => state.settings.app.name);
  const createEnabled = useGlobalStore((state) => !!state.settings.games?.createEnabled);

  const active = !!tour?.panelActive && HOME_PATHS.includes(location.pathname);
  const { data: servers } = useResource({
    queryKey: [...queryKeys.user.welcomeTour(), 'first-server'],
    queryFn: () => getServers(1),
    enabled: active,
    silent: true,
  });

  if (!active || !tour || !servers) {
    return null;
  }

  const first = servers.data[0] ?? null;
  const p = 'elements.welcomeTour.panel';

  const steps: TourStep[] = [
    { key: 'welcome', targets: [], heading: t(`${p}.welcome.heading`, { app }), body: t(`${p}.welcome.body`, {}) },
    {
      key: 'navigation',
      targets: ['#sidebar-desktop'],
      heading: t(`${p}.navigation.heading`, {}),
      body: t(`${p}.navigation.body`, {}),
      bullets: [
        t(`${p}.navigation.bullets.servers`, {}),
        t(`${p}.navigation.bullets.account`, {}),
        t(`${p}.navigation.bullets.keys`, {}),
        t(`${p}.navigation.bullets.activity`, {}),
      ],
    },
    {
      key: 'account',
      targets: ['#sidebar-account-card'],
      heading: t(`${p}.account.heading`, {}),
      body: t(`${p}.account.body`, {}),
    },
  ];
  const create: TourStep = {
    key: 'create',
    targets: ['[data-tour="create-server"]'],
    heading: t(`${p}.create.heading`, {}),
    body: t(`${p}.create.body`, {}),
  };
  if (first) {
    if (createEnabled) steps.push(create);
    steps.push({
      key: 'servers',
      targets: ['.theme-server-card'],
      heading: t(`${p}.servers.heading`, {}),
      body: t(`${p}.servers.body`, { name: first.name }),
    });
  } else {
    steps.push({
      key: 'empty',
      targets: ['#dashboard-root .mantine-Paper-root'],
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

  const finish = () => {
    update({ action: 'complete' });
    if (first) {
      navigate(`/server/${first.uuidShort}`);
    } else if (createEnabled) {
      navigate('/create');
    }
  };

  return (
    <WelcomeTour
      steps={steps}
      step={tour.step}
      onStep={(next) => update({ action: 'advance', step: next })}
      finishLabel={
        first
          ? t('elements.welcomeTour.openServer', { name: first.name })
          : createEnabled
            ? t('elements.welcomeTour.createServer', {})
            : t('elements.welcomeTour.finish', {})
      }
      onFinish={finish}
      closeLabel={t('elements.welcomeTour.skip', {})}
      onClose={() => update({ action: 'skip' })}
    />
  );
}
