// plugged.host: Admin › Status: incidents, components and overrides of the public status page
// (docs/features/status.md).
import { faBullhorn, faLayerGroup, faSliders } from '@fortawesome/free-solid-svg-icons';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import SubNavigation from '@/elements/navigation/SubNavigation.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import StatusComponents from './components/StatusComponents.tsx';
import StatusIncidents from './incidents/StatusIncidents.tsx';
import StatusOverrides from './overrides/StatusOverrides.tsx';

export default function AdminStatus() {
  const { t } = useTranslations();

  return (
    <AdminContentContainer title={t('pages.admin.status.title', {})}>
      <SubNavigation
        baseUrl='/admin/status'
        items={[
          {
            name: t('pages.admin.status.tabs.incidents.title', {}),
            icon: faBullhorn,
            path: '/',
            element: <StatusIncidents />,
          },
          {
            name: t('pages.admin.status.tabs.components.title', {}),
            icon: faLayerGroup,
            path: '/components',
            element: <StatusComponents />,
          },
          {
            name: t('pages.admin.status.tabs.overrides.title', {}),
            icon: faSliders,
            path: '/overrides',
            element: <StatusOverrides />,
          },
        ]}
      />
    </AdminContentContainer>
  );
}
