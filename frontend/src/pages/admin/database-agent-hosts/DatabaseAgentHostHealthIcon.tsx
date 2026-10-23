import { faHeart, faHeartBroken } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { z } from 'zod';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { adminDatabaseAgentHostSchema } from '@/lib/schemas/admin/databaseAgentHosts.ts';
import { adminUpgradeStatusSchema } from '@/lib/schemas/admin/system.ts';
import { useDatabaseAgentHostVersion } from '@/plugins/database-agent-hosts/useDatabaseAgentHostVersion.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function DatabaseAgentHostHealthIcon({
  databaseAgentHost,
  upgradeStatus,
}: {
  databaseAgentHost: z.infer<typeof adminDatabaseAgentHostSchema>;
  upgradeStatus?: z.infer<typeof adminUpgradeStatusSchema>;
}) {
  const { t } = useTranslations();
  const { version, unavailable, loading, upgradeAvailability } = useDatabaseAgentHostVersion(databaseAgentHost, {
    upgradeStatus,
  });

  if (loading) {
    return <Spinner size={16} />;
  }

  if (unavailable || !version) {
    return (
      <Tooltip label={t('pages.admin.databaseAgentHosts.tabs.general.page.tooltip.errorWhileFetchingVersion', {})}>
        <FontAwesomeIcon icon={faHeartBroken} className='text-(--mantine-color-red-text)' />
      </Tooltip>
    );
  }

  if (upgradeAvailability === 'available') {
    return (
      <Tooltip label={t('pages.admin.databaseAgentHosts.tabs.general.page.tooltip.updateAvailable', { version })}>
        <FontAwesomeIcon icon={faHeart} className='text-(--mantine-color-yellow-text) animate-pulse' />
      </Tooltip>
    );
  }

  if (upgradeAvailability === 'unknown') {
    return (
      <Tooltip label={t('pages.admin.databaseAgentHosts.tabs.general.page.tooltip.latestVersionUnknown', { version })}>
        <FontAwesomeIcon icon={faHeart} className='text-(--mantine-color-dimmed) animate-pulse' />
      </Tooltip>
    );
  }

  return (
    <Tooltip label={version}>
      <FontAwesomeIcon icon={faHeart} className='text-(--mantine-color-green-text) animate-pulse' />
    </Tooltip>
  );
}
