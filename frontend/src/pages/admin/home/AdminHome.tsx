import { faCog, faHeartPulse, faScaleBalanced } from '@fortawesome/free-solid-svg-icons'; // plugged.host: faScaleBalanced
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import SubNavigation from '@/elements/navigation/SubNavigation.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AdminOverviewCapacity from './capacity/AdminOverviewCapacity.tsx'; // plugged.host
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
          // plugged.host: free resources per location and the moves (docs/features/transfers.md)
          {
            name: t('pages.admin.home.tabs.capacity.title', {}),
            icon: faScaleBalanced,
            path: `/capacity`,
            element: <AdminOverviewCapacity />,
            permission: 'nodes.read',
          },
        ]}
      />
    </AdminContentContainer>
  );
}
