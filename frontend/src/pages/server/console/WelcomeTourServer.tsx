// plugged.host: the welcome tour's second part, on the first console the user opens after the
// first part, with the Pelican panel's steps (docs/features/welcome-tour.md): the console, the power
// buttons, the server's details, the setup guide's callout while it waits, the sidebar, Settings and
// the way back. A step or bullet about something this person cannot see is left out.
import WelcomeTour, { TourStep } from '@/elements/WelcomeTour.tsx';
import { isMinecraftServer } from '@/lib/domain/games.ts';
import { useMinecraftSetup } from '@/pages/server/minecraft/setup/useMinecraftSetup.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useWelcomeTour } from '@/plugins/useWelcomeTour.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

// the sidebar's rows together (not its logo, usage card or switcher), else the whole sidebar
const SIDEBAR_ROWS = ['#sidebar-desktop #sidebar-content > .overflow-y-auto > *', '#sidebar-desktop'];

type SidebarBullet = 'files' | 'content' | 'worlds' | 'gameSettings' | 'backups' | 'more' | 'moreMinecraft';

export default function WelcomeTourServer() {
  const { t } = useTranslations();
  const { tour, update } = useWelcomeTour();
  const minecraft = useServerStore((state) => isMinecraftServer(state.server.egg));
  const setup = useMinecraftSetup(!!tour?.serverActive);
  const canSetUp = useServerCan('minecraft.settings');
  const canContent = useServerCan('minecraft.content');
  const canWorlds = useServerCan(['minecraft.worlds', 'minecraft.players']);
  const canFiles = useServerCan('files.read');
  const canBackups = useServerCan('backups.read');
  // the Settings row's permissions (routers/routes/serverRoutes.ts)
  const canSettings = useServerCan(['settings.*', 'files.sftp', 'backups.read']);

  // the setup step is in the count from the start, so wait for whether the guide is waiting
  if (!tour?.serverActive || setup.isLoading) {
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
      targets: ['#console-infobar .theme-power', '#console-infobar'],
      heading: t(`${p}.power.heading`, {}),
      body: t(`${p}.power.body`, {}),
    },
    {
      key: 'details',
      targets: ['[data-tour="details"]'],
      heading: t(`${p}.details.heading`, {}),
      body: t(`${p}.details.body`, {}),
    },
  ];

  // the console's "not set up yet" callout (pages/server/minecraft/setup/SetupCallout.tsx)
  if (setup.data?.pending && canSetUp) {
    steps.push({
      key: 'setup',
      targets: ['[data-tour="setup"]'],
      heading: t(`${p}.setup.heading`, {}),
      body: t(`${p}.setup.body`, {}),
    });
  }

  // a bullet per sidebar row this person sees (elements.welcomeTour.server.sidebar.bullets)
  const bullets: (SidebarBullet | null)[] = [
    canFiles ? 'files' : null,
    minecraft && canContent ? 'content' : null,
    minecraft && canWorlds ? 'worlds' : null,
    minecraft && canSetUp ? 'gameSettings' : null,
    canBackups ? 'backups' : null,
    minecraft ? 'moreMinecraft' : 'more',
  ];
  steps.push({
    key: 'sidebar',
    targets: SIDEBAR_ROWS,
    together: true,
    heading: t(`${p}.sidebar.heading`, {}),
    body: t(`${p}.sidebar.body`, {}),
    bullets: bullets.filter((bullet) => bullet !== null).map((bullet) => t(`${p}.sidebar.bullets.${bullet}`, {})),
  });

  if (canSettings) {
    steps.push({
      key: 'settings',
      targets: ['#sidebar-desktop a[href$="/settings"]'],
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
      skipLabel={t('elements.welcomeTour.skip', {})}
      onSkip={done}
    />
  );
}
