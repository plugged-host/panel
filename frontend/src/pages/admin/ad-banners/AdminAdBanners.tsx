// plugged.host: Admin › Sponsored Banners, the banners the panel's owner sells (docs/features/ads.md#sponsored-banners)
import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Route, Routes, useNavigate, useParams } from 'react-router';
import getAdBanner from '@/api/admin/ad-banners/getAdBanner.ts';
import getAdBanners from '@/api/admin/ad-banners/getAdBanners.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adBannerTableColumns } from '@/lib/tableColumns.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AdminPermissionGuard from '@/routers/guards/AdminPermissionGuard.tsx';
import AdBannerCreateOrUpdate from './AdBannerCreateOrUpdate.tsx';
import AdBannerRow from './AdBannerRow.tsx';

function AdBannersContainer() {
  const { t } = useTranslations();
  const navigate = useNavigate();

  const {
    data: banners,
    loading,
    error,
    search,
    setSearch,
    setPage,
    setPerPage,
  } = useSearchablePaginatedTable({
    queryKey: queryKeys.admin.adBanners.all(),
    fetcher: getAdBanners,
  });

  return (
    <AdminContentContainer
      title={t('pages.admin.adBanners.title', {})}
      search={search}
      setSearch={setSearch}
      contentRight={
        <AdminCan action='ad-banners.create'>
          <Button
            onClick={() => navigate('/admin/ad-banners/new')}
            color='blue'
            leftSection={<FontAwesomeIcon icon={faPlus} />}
          >
            {t('common.button.create', {})}
          </Button>
        </AdminCan>
      }
    >
      <Table
        columns={adBannerTableColumns()}
        loading={loading}
        error={error}
        pagination={banners}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {banners?.data.map((banner) => (
          <AdBannerRow key={banner.uuid} banner={banner} />
        ))}
      </Table>
    </AdminContentContainer>
  );
}

function AdBannerView() {
  const params = useParams<'id'>();

  const resource = useResource({
    queryKey: queryKeys.admin.adBanners.detail(params.id!),
    queryFn: () => getAdBanner(params.id!),
  });

  return (
    <ResourceView resource={resource}>{(banner) => <AdBannerCreateOrUpdate contextBanner={banner} />}</ResourceView>
  );
}

export default function AdminAdBanners() {
  return (
    <Routes>
      <Route path='/' element={<AdBannersContainer />} />
      <Route path='/:id' element={<AdBannerView />} />
      <Route element={<AdminPermissionGuard permission='ad-banners.create' />}>
        <Route path='/new' element={<AdBannerCreateOrUpdate />} />
      </Route>
    </Routes>
  );
}
