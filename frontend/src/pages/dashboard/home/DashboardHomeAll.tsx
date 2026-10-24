import { faSearch } from '@fortawesome/free-solid-svg-icons'; // plugged.host
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome'; // plugged.host
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts'; // plugged.host: without getEmptyPaginationSet
import getServerGroups from '@/api/me/servers/groups/getServerGroups.ts';
import getServers from '@/api/server/getServers.ts';
import { AdPageSlot } from '@/elements/ads/AdPlacements.tsx'; // plugged.host
import { AdminCan } from '@/elements/Can.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import { NoItems, Pagination } from '@/elements/data-display/Table.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import ErrorState from '@/elements/feedback/ErrorState.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import { ObjectSet } from '@/lib/objectSet.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { eventKeyMatches } from '@/lib/quickActions/shortcuts.ts';
import { serverPowerAction, serverSchema } from '@/lib/schemas/server/server.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useBulkPowerActions } from '@/plugins/server/useBulkPowerActions.ts';
import { useServerListShowOthers } from '@/plugins/server/useServerListShowOthers.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useUserStore } from '@/stores/user.ts';
import CreateServerButton from '../create/CreateServerButton.tsx'; // plugged.host
import BulkActionBar from './BulkActionBar.tsx';
import ServerItem from './ServerItem.tsx';

export default function DashboardHomeAll() {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const setServerGroups = useUserStore((state) => state.setServerGroups);
  const [serverListShowOthers, setServerListShowOthers] = useServerListShowOthers();
  const { addToast } = useToast();

  const [selectedServers, setSelectedServers] = useState(new ObjectSet<z.infer<typeof serverSchema>, 'uuid'>('uuid'));
  const sKeyPressedRef = useRef(false);

  const { handleBulkPowerAction, bulkActionLoading } = useBulkPowerActions();

  useEffect(() => {
    getServerGroups()
      .then((response) => {
        setServerGroups(response);
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      });
  }, [addToast, setServerGroups]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (eventKeyMatches(e, 's')) {
        const target = e.target as HTMLElement;
        if (target.tagName !== 'INPUT' && target.tagName !== 'TEXTAREA' && !target.isContentEditable) {
          sKeyPressedRef.current = true;
        }
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (eventKeyMatches(e, 's')) {
        sKeyPressedRef.current = false;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);

  const {
    data: servers,
    loading,
    error,
    refetch,
    search,
    debouncedSearch,
    setSearch,
    setPage,
    setPerPage, // plugged.host
  } = useSearchablePaginatedTable({
    queryKey: queryKeys.user.servers.all(),
    fetcher: (page, search, perPage) => getServers(page, search, serverListShowOthers, perPage), // plugged.host: per page
    deps: [serverListShowOthers],
  });

  const handleServerSelectionChange = (server: z.infer<typeof serverSchema>, selected: boolean) => {
    setSelectedServers((prev) => {
      const newSet = prev.clone();
      if (selected) {
        newSet.add(server);
      } else {
        newSet.delete(server);
      }
      return newSet;
    });
  };

  const handleServerClick = (server: z.infer<typeof serverSchema>, event: React.MouseEvent) => {
    if (sKeyPressedRef.current) {
      event.preventDefault();
      event.stopPropagation();
      handleServerSelectionChange(server, !selectedServers.has(server));
    }
  };

  const onBulkAction = async (action: z.infer<typeof serverPowerAction>) => {
    await handleBulkPowerAction(selectedServers.keys(), action);
    setSelectedServers(new ObjectSet('uuid'));
  };

  return (
    <AccountContentContainer
      title={t('pages.account.home.title', {})}
      registry={window.extensionContext.extensionRegistry.pages.dashboard.home.containerAll}
    >
      {/* plugged.host: the Pelican server list (ListServers under Theme v2): no tabs, one toolbar row
          over the cards with Create server at its start and the search at its end, the pagination
          only under the cards. Bulk selection stays on S + click (BulkActionBar). */}
      <div className='theme-toolbar'>
        <CreateServerButton />
        <div className='theme-toolbar-end'>
          <TextInput
            placeholder={t('common.input.search', {})}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onKeyDown={(e) => {
              const firstServer = servers?.data[0];
              if (e.key === 'Enter' && firstServer && !loading && search === debouncedSearch) {
                e.preventDefault();
                navigate(`/server/${firstServer.uuidShort}`);
              }
            }}
            leftSection={<FontAwesomeIcon icon={faSearch} />}
            className='theme-header-search'
          />
          <AdminCan action='servers.read'>
            <Switch
              label={t('pages.account.home.tabs.allServers.page.input.showOtherUsersServers', {})}
              checked={serverListShowOthers}
              onChange={(e) => {
                setPage(1);
                setServerListShowOthers(e.currentTarget.checked);
              }}
            />
          </AdminCan>
        </div>
      </div>
      {loading ? (
        <Spinner.Centered />
      ) : error && !servers ? (
        <ErrorState error={error} onRetry={refetch} />
      ) : servers?.total === 0 ? (
        debouncedSearch ? (
          <NoItems />
        ) : (
          <div className='theme-server-empty' data-tour='servers-empty'>
            <EmptyState flush title={t('pages.account.home.emptyAccess', {})} />
          </div>
        )
      ) : (
        <div className='theme-server-grid'>
          {servers?.data.map((server) => (
            <ServerItem
              key={server.uuid}
              server={server}
              showContextMenu
              showForeignServerBadge
              showSelection={false}
              isSelected={selectedServers.has(server.uuid)}
              onSelectionChange={(selected) => handleServerSelectionChange(server, selected)}
              onClick={(e) => handleServerClick(server, e)}
              sKeyPressedRef={sKeyPressedRef}
            />
          ))}
        </div>
      )}
      <BulkActionBar
        selectedCount={selectedServers.size}
        onClear={() => setSelectedServers(new ObjectSet('uuid'))}
        onAction={onBulkAction}
        loading={bulkActionLoading}
      />
      {servers && servers.total > 0 && (
        <div className='theme-list-foot'>
          <Pagination data={servers} onPageSelect={setPage} onPerPageSelect={setPerPage} />
        </div>
      )}
      <AdPageSlot /> {/* plugged.host: docs/features/ads.md */}
    </AccountContentContainer>
  );
}
