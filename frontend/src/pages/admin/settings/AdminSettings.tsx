import {
  faAt,
  faBan,
  faCircleQuestion, // plugged.host
  faClockRotateLeft, // plugged.host
  faDatabase,
  faEnvelopesBulk,
  faGamepad,
  faHandshake,
  faHeartPulse,
  faLayerGroup,
  faRobot,
  faScaleBalanced,
  faServer,
  faShareNodes,
  faShieldHalved,
  faToolbox,
  faUser,
  faUserCheck,
} from '@fortawesome/free-solid-svg-icons';
import { useEffect, useState } from 'react';
import getSettings from '@/api/admin/settings/getSettings.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import FormDraftScope from '@/elements/FormDraftScope.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import SubNavigation from '@/elements/navigation/SubNavigation.tsx';
import EmailTemplatesContainer from '@/pages/admin/settings/emailTemplates/EmailTemplatesContainer.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { trackActivity } from '@/stores/activity.ts';
import { useAdminStore } from '@/stores/admin.tsx';
import AbuseContainer from './abuse/AbuseContainer.tsx';
import ActivityContainer from './activity/ActivityContainer.tsx';
import ApplicationContainer from './application/ApplicationContainer.tsx';
import CaptchaContainer from './captcha/CaptchaContainer.tsx';
import EmailContainer from './email/EmailContainer.tsx';
import GamesContainer from './games/GamesContainer.tsx'; // plugged.host
import MetadataContainer from './metadata/MetadataContainer.tsx';
import NoprofitContainer from './noprofit/NoprofitContainer.tsx';
import PlacementContainer from './placement/PlacementContainer.tsx'; // plugged.host
import RatelimitsContainer from './ratelimits/RatelimitsContainer.tsx';
import RecoveryContainer from './recovery/RecoveryContainer.tsx'; // plugged.host
import ServerContainer from './server/ServerContainer.tsx';
import StatusContainer from './status/StatusContainer.tsx';
import StorageContainer from './storage/StorageContainer.tsx';
import TourContainer from './tour/TourContainer.tsx'; // plugged.host
import UserContainer from './user/UserContainer.tsx';
import WebauthnContainer from './webauthn/WebauthnContainer.tsx';

export default function AdminSettings() {
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

  return loading ? (
    <Spinner.Centered />
  ) : (
    <AdminContentContainer title={t('pages.admin.settings.title', {})}>
      <FormDraftScope baseUrl='/admin/settings'>
        <SubNavigation
          baseUrl='/admin/settings'
          items={[
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
            {
              name: t('pages.admin.settings.tabs.mail.title', {}),
              icon: faAt,
              path: '/mail',
              element: <EmailContainer />,
            },
            {
              name: t('pages.admin.settings.tabs.mailTemplates.title', {}),
              icon: faEnvelopesBulk,
              path: '/mail-templates',
              element: <EmailTemplatesContainer />,
              permission: 'email-templates.read',
            },
            {
              name: t('pages.admin.settings.tabs.captcha.title', {}),
              icon: faRobot,
              path: '/captcha',
              element: <CaptchaContainer />,
            },
            // plugged.host: noprofit.space, next to the other sign-in settings
            {
              name: t('pages.admin.settings.tabs.noprofit.title', {}),
              icon: faHandshake,
              path: '/noprofit',
              element: <NoprofitContainer />,
            },
            {
              name: t('pages.admin.settings.tabs.webauthn.title', {}),
              icon: faUserCheck,
              path: '/webauthn',
              element: <WebauthnContainer />,
            },
            {
              name: t('pages.admin.settings.tabs.server.title', {}),
              icon: faServer,
              path: '/server',
              element: <ServerContainer />,
            },
            {
              name: t('pages.admin.settings.tabs.user.title', {}),
              icon: faUser,
              path: '/user',
              element: <UserContainer />,
            },
            {
              name: t('pages.admin.settings.tabs.activity.title', {}),
              icon: faToolbox,
              path: '/activity',
              element: <ActivityContainer />,
            },
            {
              name: t('pages.admin.settings.tabs.ratelimits.title', {}),
              icon: faBan,
              path: '/ratelimits',
              element: <RatelimitsContainer />,
            },
            // plugged.host: the public status page
            {
              name: t('pages.admin.settings.tabs.status.title', {}),
              icon: faHeartPulse,
              path: '/status',
              element: <StatusContainer />,
            },
            // plugged.host: abuse protection
            {
              name: t('pages.admin.settings.tabs.abuse.title', {}),
              icon: faShieldHalved,
              path: '/abuse',
              element: <AbuseContainer />,
            },
            // plugged.host: what players may create, the deployment pause
            {
              name: t('pages.admin.settings.tabs.games.title', {}),
              icon: faGamepad,
              path: '/games',
              element: <GamesContainer />,
            },
            {
              name: t('pages.admin.settings.tabs.placement.title', {}),
              icon: faScaleBalanced,
              path: '/placement',
              element: <PlacementContainer />,
            },
            // plugged.host: the welcome tour
            {
              name: t('pages.admin.settings.tabs.tour.title', {}),
              icon: faCircleQuestion,
              path: '/tour',
              element: <TourContainer />,
            },
            // plugged.host: deleted servers' copies
            {
              name: t('pages.admin.settings.tabs.recovery.title', {}),
              icon: faClockRotateLeft,
              path: '/recovery',
              element: <RecoveryContainer />,
            },
          ]}
        />
      </FormDraftScope>
    </AdminContentContainer>
  );
}
