// plugged.host: the welcome tour's second part, on the first console the user opens after the
// first part (docs/features/welcome-tour.md).
import WelcomeTour, { TourStep } from '@/elements/WelcomeTour.tsx';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useWelcomeTour } from '@/plugins/useWelcomeTour.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function WelcomeTourServer() {
  const { t } = useTranslations();
  const { tour, update } = useWelcomeTour();
  const canSettings = useServerCan(['settings.*']);

  if (!tour?.serverActive) {
    return null;
  }

  const p = 'elements.welcomeTour.server';
  const steps: TourStep[] = [
    {
      key: 'console',
      targets: ['[data-tour="console"]'],
      heading: t(`${p}.console.heading`, {}),
      body: t(`${p}.console.body`, {}),
    },
    {
      key: 'power',
      targets: ['#console-infobar'],
      heading: t(`${p}.power.heading`, {}),
      body: t(`${p}.power.body`, {}),
    },
    {
      key: 'details',
      targets: ['[data-tour="details"]'],
      heading: t(`${p}.details.heading`, {}),
      body: t(`${p}.details.body`, {}),
    },
    {
      key: 'sidebar',
      targets: ['#sidebar-desktop'],
      heading: t(`${p}.sidebar.heading`, {}),
      body: t(`${p}.sidebar.body`, {}),
      bullets: [
        t(`${p}.sidebar.bullets.files`, {}),
        t(`${p}.sidebar.bullets.backups`, {}),
        t(`${p}.sidebar.bullets.network`, {}),
        t(`${p}.sidebar.bullets.more`, {}),
      ],
    },
  ];
  if (canSettings) {
    steps.push({
      key: 'settings',
      targets: ['#sidebar-desktop a.sidebar-link[href$="/settings"]'],
      heading: t(`${p}.settings.heading`, {}),
      body: t(`${p}.settings.body`, {}),
    });
  }
  steps.push({
    key: 'home',
    targets: ['#sidebar-desktop .theme-sidebar-logo'],
    heading: t(`${p}.home.heading`, {}),
    body: t(`${p}.home.body`, {}),
  });

  const done = () => update({ action: 'complete_server' });

  return (
    <WelcomeTour
      steps={steps}
      step={tour.serverStep}
      onStep={(next) => update({ action: 'advance_server', step: next })}
      finishLabel={t('elements.welcomeTour.finish', {})}
      onFinish={done}
      closeLabel={t('elements.welcomeTour.notNow', {})}
      onClose={done}
    />
  );
}
