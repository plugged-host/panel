import { faDiscord } from '@fortawesome/free-brands-svg-icons'; // plugged.host
import {
  faAt,
  faBan,
  faBoxArchive, // plugged.host
  faCar, // plugged.host
  faCircleQuestion, // plugged.host
  faCircleUp,
  faClockRotateLeft, // plugged.host
  faCode, // plugged.host
  faCoins, // plugged.host
  faComments, // plugged.host
  faCube, // plugged.host
  faCubes, // plugged.host
  faDatabase,
  faEnvelopesBulk,
  faEyeSlash, // plugged.host
  faGamepad,
  faGavel, // plugged.host
  faGlobe, // plugged.host
  faHandshake,
  faHeartPulse,
  faHourglassHalf, // plugged.host
  faLayerGroup,
  faLock, // plugged.host
  faMedal, // plugged.host
  faRectangleAd, // plugged.host
  faRobot,
  faScaleBalanced,
  faServer,
  faShareNodes,
  faShieldHalved,
  faStore, // plugged.host
  faTableColumns, // plugged.host
  faToolbox,
  faUser,
  faUserCheck,
  faUserGear, // plugged.host
  faWindowMaximize, // plugged.host
  faWrench, // plugged.host
  IconDefinition, // plugged.host
} from '@fortawesome/free-solid-svg-icons';
import { useEffect, useState } from 'react';
import { Navigate, useParams } from 'react-router'; // plugged.host
import getSettings from '@/api/admin/settings/getSettings.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import FormDraftScope from '@/elements/FormDraftScope.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import SubNavigation, { type ItemProp } from '@/elements/navigation/SubNavigation.tsx'; // plugged.host
import { type LazyString, resolveString } from '@/lib/lazy.ts'; // plugged.host
import EmailTemplatesContainer from '@/pages/admin/settings/emailTemplates/EmailTemplatesContainer.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { getTranslations, useTranslations } from '@/providers/TranslationProvider.tsx';
import { trackActivity } from '@/stores/activity.ts';
import { useAdminStore } from '@/stores/admin.tsx';
import AbuseContainer from './abuse/AbuseContainer.tsx';
import ActivityContainer from './activity/ActivityContainer.tsx';
import AdsContainer from './ads/AdsContainer.tsx'; // plugged.host
import ApplicationContainer from './application/ApplicationContainer.tsx';
import CaptchaContainer from './captcha/CaptchaContainer.tsx';
import DevboxesContainer from './devboxes/DevboxesContainer.tsx'; // plugged.host
import DiscordContainer from './discord/DiscordContainer.tsx'; // plugged.host
import EconomyContainer from './economy/EconomyContainer.tsx'; // plugged.host
import EmailContainer from './email/EmailContainer.tsx';
import FivemContainer from './fivem/FivemContainer.tsx'; // plugged.host
import GamesContainer from './games/GamesContainer.tsx'; // plugged.host
import LimboContainer from './limbo/LimboContainer.tsx'; // plugged.host
import MetadataContainer from './metadata/MetadataContainer.tsx';
import MinecraftContainer from './minecraft/MinecraftContainer.tsx'; // plugged.host
import NeoContainer from './neo/NeoContainer.tsx'; // plugged.host
import NeoDiscordContainer from './neoDiscord/NeoDiscordContainer.tsx'; // plugged.host
import NoprofitContainer from './noprofit/NoprofitContainer.tsx';
import PlacementContainer from './placement/PlacementContainer.tsx'; // plugged.host
import QueueContainer from './queue/QueueContainer.tsx'; // plugged.host
import RatelimitsContainer from './ratelimits/RatelimitsContainer.tsx';
import RecoveryContainer from './recovery/RecoveryContainer.tsx'; // plugged.host
import RuxContainer from './rux/RuxContainer.tsx'; // plugged.host
import ServerContainer from './server/ServerContainer.tsx';
import StatusContainer from './status/StatusContainer.tsx';
import StorageContainer from './storage/StorageContainer.tsx';
import SubdomainsContainer from './subdomains/SubdomainsContainer.tsx'; // plugged.host
import TourContainer from './tour/TourContainer.tsx'; // plugged.host
import UpdatesContainer from './updates/UpdatesContainer.tsx';
import UserContainer from './user/UserContainer.tsx';
import WebauthnContainer from './webauthn/WebauthnContainer.tsx';

// plugged.host: the settings are split into a few pages, each its own entry under "Settings" in the
// admin sidebar (docs/upstream.md), instead of one page with every tab.
export type SettingsGroupKey = 'general' | 'mail' | 'accounts' | 'hosting' | 'security' | 'economy' | 'ads' | 'ai';

type Translate = ReturnType<typeof useTranslations>['t'];

export interface SettingsGroup {
  key: SettingsGroupKey;
  icon: IconDefinition;
  name: LazyString;
  tabs: (t: Translate) => ItemProp[];
}

export const settingsGroups: SettingsGroup[] = [
  {
    key: 'general',
    icon: faWrench,
    name: () => getTranslations().t('pages.admin.settings.groups.general', {}),
    tabs: (t) => [
      {
        name: t('pages.admin.settings.tabs.application.title', {}),
        icon: faLayerGroup,
        path: '/',
        element: <ApplicationContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.metadata.title', {}),
        icon: faShareNodes,
        path: '/metadata',
        element: <MetadataContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.storage.title', {}),
        icon: faDatabase,
        path: '/storage',
        element: <StorageContainer />,
      },
      // the public status page
      {
        name: t('pages.admin.settings.tabs.status.title', {}),
        icon: faHeartPulse,
        path: '/status',
        element: <StatusContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.updates.title', {}),
        icon: faCircleUp,
        path: '/updates',
        element: <UpdatesContainer />,
      },
    ],
  },
  {
    key: 'mail',
    icon: faAt,
    name: () => getTranslations().t('pages.admin.settings.groups.mail', {}),
    tabs: (t) => [
      {
        name: t('pages.admin.settings.tabs.mail.title', {}),
        icon: faAt,
        path: '/',
        element: <EmailContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.mailTemplates.title', {}),
        icon: faEnvelopesBulk,
        path: '/templates',
        element: <EmailTemplatesContainer />,
        permission: 'email-templates.read',
      },
    ],
  },
  {
    key: 'accounts',
    icon: faUserGear,
    name: () => getTranslations().t('pages.admin.settings.groups.accounts', {}),
    tabs: (t) => [
      {
        name: t('pages.admin.settings.tabs.user.title', {}),
        icon: faUser,
        path: '/',
        element: <UserContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.noprofit.title', {}),
        icon: faHandshake,
        path: '/noprofit',
        element: <NoprofitContainer />,
      },
      // plugged.host: Discord membership, invite and boost rewards, the bots (docs/features/discord.md)
      {
        name: t('pages.admin.settings.tabs.discord.title', {}),
        icon: faDiscord,
        path: '/discord',
        element: <DiscordContainer />,
      },
      // plugged.host: rux, the moderation bot (docs/features/rux.md)
      {
        name: t('pages.admin.settings.tabs.rux.title', {}),
        icon: faGavel,
        path: '/rux',
        element: <RuxContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.webauthn.title', {}),
        icon: faUserCheck,
        path: '/webauthn',
        element: <WebauthnContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.tour.title', {}),
        icon: faCircleQuestion,
        path: '/tour',
        element: <TourContainer />,
      },
    ],
  },
  {
    key: 'hosting',
    icon: faCubes,
    name: () => getTranslations().t('pages.admin.settings.groups.hosting', {}),
    tabs: (t) => [
      {
        name: t('pages.admin.settings.tabs.server.title', {}),
        icon: faServer,
        path: '/',
        element: <ServerContainer />,
      },
      // what players may create, the deployment pause, deleted servers' copies
      {
        name: t('pages.admin.settings.tabs.games.title', {}),
        icon: faGamepad,
        path: '/games',
        element: <GamesContainer />,
      },
      // plugged.host: the Minecraft pages' sources (docs/features/minecraft.md)
      {
        name: t('pages.admin.settings.tabs.minecraft.title', {}),
        icon: faCube,
        path: '/minecraft',
        element: <MinecraftContainer />,
      },
      // plugged.host: the image and edition FiveM servers run (docs/features/fivem.md)
      {
        name: t('pages.admin.settings.tabs.fivem.title', {}),
        icon: faCar,
        path: '/fivem',
        element: <FivemContainer />,
      },
      // plugged.host: server subdomains on Cloudflare (docs/features/subdomains.md)
      {
        name: t('pages.admin.settings.tabs.subdomains.title', {}),
        icon: faGlobe,
        path: '/subdomains',
        element: <SubdomainsContainer />,
      },
      // plugged.host: VS Code devboxes next to game servers (docs/features/devboxes.md)
      {
        name: t('pages.admin.settings.tabs.devboxes.title', {}),
        icon: faCode,
        path: '/devboxes',
        element: <DevboxesContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.placement.title', {}),
        icon: faScaleBalanced,
        path: '/placement',
        element: <PlacementContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.queue.title', {}),
        icon: faHourglassHalf,
        path: '/queue',
        element: <QueueContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.limbo.title', {}),
        icon: faBoxArchive,
        path: '/limbo',
        element: <LimboContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.recovery.title', {}),
        icon: faClockRotateLeft,
        path: '/recovery',
        element: <RecoveryContainer />,
      },
    ],
  },
  {
    key: 'security',
    icon: faLock,
    name: () => getTranslations().t('pages.admin.settings.groups.security', {}),
    tabs: (t) => [
      {
        name: t('pages.admin.settings.tabs.captcha.title', {}),
        icon: faRobot,
        path: '/',
        element: <CaptchaContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.ratelimits.title', {}),
        icon: faBan,
        path: '/ratelimits',
        element: <RatelimitsContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.abuse.title', {}),
        icon: faShieldHalved,
        path: '/abuse',
        element: <AbuseContainer />,
      },
      {
        name: t('pages.admin.settings.tabs.activity.title', {}),
        icon: faToolbox,
        path: '/activity',
        element: <ActivityContainer />,
      },
    ],
  },
  {
    key: 'economy',
    icon: faCoins,
    name: () => getTranslations().t('pages.admin.settings.groups.economy', {}),
    tabs: (t) => [
      {
        name: t('pages.admin.settings.tabs.economy.store', {}),
        icon: faStore,
        path: '/',
        element: <EconomyContainer part='store' />,
      },
      {
        name: t('pages.admin.settings.tabs.economy.tokens', {}),
        icon: faCoins,
        path: '/tokens',
        element: <EconomyContainer part='tokens' />,
      },
      {
        name: t('pages.admin.settings.tabs.economy.memberships', {}),
        icon: faMedal,
        path: '/memberships',
        element: <EconomyContainer part='memberships' />,
      },
    ],
  },
  {
    key: 'ads',
    icon: faRectangleAd,
    name: () => getTranslations().t('pages.admin.settings.groups.ads', {}),
    tabs: (t) => [
      {
        name: t('pages.admin.settings.tabs.ads.placement', {}),
        icon: faTableColumns,
        path: '/',
        element: <AdsContainer part='placement' />,
      },
      {
        name: t('pages.admin.settings.tabs.ads.adsterra', {}),
        icon: faRectangleAd,
        path: '/adsterra',
        element: <AdsContainer part='adsterra' />,
      },
      {
        name: t('pages.admin.settings.tabs.ads.monetag', {}),
        icon: faWindowMaximize,
        path: '/monetag',
        element: <AdsContainer part='monetag' />,
      },
      {
        name: t('pages.admin.settings.tabs.ads.room', {}),
        icon: faHourglassHalf,
        path: '/waiting-room',
        element: <AdsContainer part='room' />,
      },
      {
        name: t('pages.admin.settings.tabs.ads.adblock', {}),
        icon: faEyeSlash,
        path: '/ad-blockers',
        element: <AdsContainer part='adblock' />,
      },
      {
        name: t('pages.admin.settings.tabs.ads.inGame', {}),
        icon: faGamepad,
        path: '/in-game',
        element: <AdsContainer part='inGame' />,
      },
    ],
  },
  // plugged.host: the AI settings, neo's tab (docs/features/neo.md)
  {
    key: 'ai',
    icon: faRobot,
    name: () => getTranslations().t('pages.admin.settings.groups.ai', {}),
    tabs: (t) => [
      {
        name: t('pages.admin.settings.tabs.neo.title', {}),
        icon: faRobot,
        path: '/',
        element: <NeoContainer />,
      },
      // neo on Discord, account support (docs/features/neo-discord.md)
      {
        name: t('pages.admin.settings.tabs.neoDiscord.title', {}),
        icon: faComments,
        path: '/neo-discord',
        element: <NeoDiscordContainer />,
      },
    ],
  },
];

// plugged.host: where the tabs of the single settings page went, for old links and bookmarks
const movedTabs: Record<string, string> = {
  metadata: 'general/metadata',
  storage: 'general/storage',
  status: 'general/status',
  updates: 'general/updates',
  'mail-templates': 'mail/templates',
  user: 'accounts',
  noprofit: 'accounts/noprofit',
  webauthn: 'accounts/webauthn',
  tour: 'accounts/tour',
  server: 'hosting',
  games: 'hosting/games',
  placement: 'hosting/placement',
  recovery: 'hosting/recovery',
  captcha: 'security',
  ratelimits: 'security/ratelimits',
  abuse: 'security/abuse',
  activity: 'security/activity',
};

export function AdminSettingsRedirect() {
  const tab = (useParams()['*'] ?? '').split('/')[0];

  return <Navigate to={`/admin/settings/${movedTabs[tab] ?? 'general'}`} replace />;
}

export default function AdminSettings({ group }: { group: SettingsGroup }) {
  const { addToast } = useToast();
  const { t } = useTranslations();
  const setSettings = useAdminStore((state) => state.setSettings);

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    trackActivity(getSettings())
      .then(setSettings)
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      })
      .finally(() => setLoading(false));
  }, []);

  const baseUrl = `/admin/settings/${group.key}`;

  return loading ? (
    <Spinner.Centered />
  ) : (
    <AdminContentContainer title={resolveString(group.name)}>
      <FormDraftScope baseUrl={baseUrl}>
        <SubNavigation baseUrl={baseUrl} items={group.tabs(t)} />
      </FormDraftScope>
    </AdminContentContainer>
  );
}
