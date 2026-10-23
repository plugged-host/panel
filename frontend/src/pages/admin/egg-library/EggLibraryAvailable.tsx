import { faDownload } from '@fortawesome/free-solid-svg-icons';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import getAllEggRepositoryEggs from '@/api/admin/egg-repositories/eggs/getAllEggRepositoryEggs.ts';
import Button from '@/elements/buttons/Button.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { NoItems, Pagination } from '@/elements/data-display/Table.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import ErrorState from '@/elements/feedback/ErrorState.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminEggRepositoryCatalogEggSchema } from '@/lib/schemas/admin/eggRepositories.ts';
import EggRepositoryEggDrawer from '@/pages/admin/egg-repositories/eggs/drawers/EggRepositoryEggDrawer.tsx';
import EggRepositoryEggInstallModal from '@/pages/admin/egg-repositories/eggs/modals/EggRepositoryEggInstallModal.tsx';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AvailableEggCard from './AvailableEggCard.tsx';
import EggCardGrid from './EggCardGrid.tsx';

type CatalogEgg = z.infer<typeof adminEggRepositoryCatalogEggSchema>;

export default function EggLibraryAvailable() {
  const { t } = useTranslations();
  const navigate = useNavigate();

  const [installItem, setInstallItem] = useState<CatalogEgg | null>(null);
  const [installOpen, setInstallOpen] = useState(false);
  const [readmeItem, setReadmeItem] = useState<CatalogEgg | null>(null);
  const [readmeOpen, setReadmeOpen] = useState(false);

  const { data, loading, error, search, debouncedSearch, setSearch, setPage, refetch } = useSearchablePaginatedTable({
    queryKey: queryKeys.admin.eggRepositories.eggsUnscoped(),
    fetcher: (page, search) => getAllEggRepositoryEggs(page, search),
  });

  const isEmpty = !loading && !error && !debouncedSearch && data?.total === 0;

  return (
    <AdminSubContentContainer
      title={t('pages.admin.eggLibrary.tabs.available.page.title', {})}
      titleOrder={2}
      search={isEmpty ? undefined : search}
      setSearch={isEmpty ? undefined : setSearch}
    >
      <EggRepositoryEggInstallModal
        eggRepository={installItem?.egg.eggRepository ?? null}
        eggs={installItem ? [installItem.egg] : []}
        opened={installOpen}
        onClose={() => setInstallOpen(false)}
        onInstalled={refetch}
      />

      <EggRepositoryEggDrawer
        eggRepository={readmeItem?.egg.eggRepository ?? null}
        egg={readmeItem?.egg ?? null}
        opened={readmeOpen}
        onClose={() => setReadmeOpen(false)}
      />

      {isEmpty ? (
        <EmptyState
          icon={faDownload}
          title={t('pages.admin.eggLibrary.tabs.available.page.empty.title', {})}
          description={t('pages.admin.eggLibrary.tabs.available.page.empty.description', {})}
        >
          <Button onClick={() => navigate('/admin/egg-repositories')}>
            {t('pages.admin.eggLibrary.tabs.available.page.empty.button', {})}
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
              <AvailableEggCard
                key={item.egg.uuid}
                item={item}
                onInstall={() => {
                  setInstallItem(item);
                  setInstallOpen(true);
                }}
                onReadme={() => {
                  setReadmeItem(item);
                  setReadmeOpen(true);
                }}
              />
            ))}
          </EggCardGrid>
          <Pagination data={data} onPageSelect={setPage} />
        </Stack>
      )}
    </AdminSubContentContainer>
  );
}
