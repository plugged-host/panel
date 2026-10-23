import { faHeart, faHeartBroken } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { z } from 'zod';
import getDatabaseHostVersion from '@/api/admin/database-hosts/getDatabaseHostVersion.ts';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminDatabaseHostSchema } from '@/lib/schemas/admin/databaseHosts.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function DatabaseHostHealthIcon({
  databaseHost,
}: {
  databaseHost: z.infer<typeof adminDatabaseHostSchema>;
}) {
  const { t } = useTranslations();
  const {
    data: version,
    error,
    loading,
  } = useResource({
    queryKey: queryKeys.admin.databaseHosts.version(databaseHost.uuid),
    queryFn: () => getDatabaseHostVersion(databaseHost.uuid),
    silent: true,
  });

  if (loading && version === undefined && !error) {
    return <Spinner size={16} />;
  }

  if (error || !version) {
    return (
      <Tooltip label={t('pages.admin.databaseHosts.tabs.general.page.tooltip.errorWhileFetchingVersion', {})}>
        <FontAwesomeIcon icon={faHeartBroken} className='text-(--mantine-color-red-text)' />
      </Tooltip>
    );
  }

  return (
    <Tooltip label={version}>
      <FontAwesomeIcon icon={faHeart} className='text-(--mantine-color-green-text) animate-pulse' />
    </Tooltip>
  );
}
