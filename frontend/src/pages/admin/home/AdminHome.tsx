import { faCog, faHeartPulse } from '@fortawesome/free-solid-svg-icons';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import SubNavigation from '@/elements/navigation/SubNavigation.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AdminOverviewHealth from './health/AdminOverviewHealth.tsx';
import AdminOverview from './overview/AdminOverview.tsx';

export default function AdminHome() {
  const { t } = useTranslations();

  return (
    <AdminContentContainer
      title={t('pages.account.admin.title', {})}
      registry={window.extensionContext.extensionRegistry.pages.admin.home.container}
    >
      <SubNavigation
        baseUrl='/admin'
        registry={window.extensionContext.extensionRegistry.pages.admin.home.subNavigation}
        registryProps={{}}
        items={[
          {
            name: t('pages.admin.home.tabs.overview.title', {}),
            icon: faCog,
            path: `/`,
            element: <AdminOverview />,
          },
          // plugged.host: no update checker (docs/upstream.md)
          {
            name: t('pages.admin.home.tabs.health.title', {}),
            icon: faHeartPulse,
            path: `/health`,
            element: <AdminOverviewHealth />,
            permission: 'stats.read',
          },
        ]}
      />
    </AdminContentContainer>
  );
}
