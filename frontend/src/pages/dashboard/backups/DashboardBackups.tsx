// plugged.host: the account's Backups page: the newest copy of each server, and of servers deleted
// in the last days (docs/features/backups-page.md).
import getAccountBackups from '@/api/me/backups/getAccountBackups.ts';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AccountBackupRow from './AccountBackupRow.tsx';

export default function DashboardBackups() {
  const { t } = useTranslations();
  const page = 'pages.account.backups';

  const backups = useResource({
    queryKey: queryKeys.user.backups(),
    queryFn: getAccountBackups,
  });

  return (
    <AccountContentContainer title={t(`${page}.title`, {})}>
      <Text size='sm' c='dimmed' mb='md'>
        {t(`${page}.description`, {})}
      </Text>

      <ResourceView resource={backups}>
        {(rows) =>
          rows.length === 0 ? (
            <Text size='sm' c='dimmed'>
              {t(`${page}.empty`, {})}
            </Text>
          ) : (
            <Table
              columns={[
                t(`${page}.table.server`, {}),
                t(`${page}.table.copy`, {}),
                t('common.table.columns.size', {}),
                t(`${page}.table.keptUntil`, {}),
                '',
              ]}
            >
              {rows.map((row) => (
                <AccountBackupRow key={row.serverUuid} row={row} />
              ))}
            </Table>
          )
        }
      </ResourceView>
    </AccountContentContainer>
  );
}
