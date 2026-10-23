import { faEgg } from '@fortawesome/free-solid-svg-icons';
import { useNavigate } from 'react-router';
import searchEggs from '@/api/admin/eggs/searchEggs.ts';
import Button from '@/elements/buttons/Button.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { NoItems, Pagination } from '@/elements/data-display/Table.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import ErrorState from '@/elements/feedback/ErrorState.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import EggCardGrid from './EggCardGrid.tsx';
import InstalledEggCard from './InstalledEggCard.tsx';

export default function EggLibraryInstalled() {
  const { t } = useTranslations();
  const navigate = useNavigate();

  const { data, loading, error, search, debouncedSearch, setSearch, setPage, refetch } = useSearchablePaginatedTable({
    queryKey: queryKeys.admin.eggs.list(),
    fetcher: (page, search) => searchEggs(null, page, search),
  });

  const isEmpty = !loading && !error && !debouncedSearch && data?.total === 0;

  return (
    <AdminSubContentContainer
      title={t('pages.admin.eggLibrary.tabs.installed.page.title', {})}
      titleOrder={2}
      search={isEmpty ? undefined : search}
      setSearch={isEmpty ? undefined : setSearch}
    >
      {isEmpty ? (
        <EmptyState
          icon={faEgg}
          title={t('pages.admin.eggLibrary.tabs.installed.page.empty.title', {})}
          description={t('pages.admin.eggLibrary.tabs.installed.page.empty.description', {})}
        >
          <Button onClick={() => navigate('/admin/egg-library/available')}>
            {t('pages.admin.eggLibrary.tabs.installed.page.empty.button', {})}
          </Button>
        </EmptyState>
      ) : error ? (
        <ErrorState error={error} />
      ) : !data ? (
        <Spinner.Centered />
      ) : data.total === 0 ? (
        <NoItems />
      ) : (
        <Stack>
          <EggCardGrid>
            {data.data.map((item) => (
              <InstalledEggCard key={item.egg.uuid} egg={item.egg} nest={item.nest} onUpdated={refetch} />
            ))}
          </EggCardGrid>
          <Pagination data={data} onPageSelect={setPage} />
        </Stack>
      )}
    </AdminSubContentContainer>
  );
}
