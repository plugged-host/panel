// plugged.host: the account's Backups page, laid out like the Pelican panel's backups portal: the
// user's servers as full-width rows with the newest copy of each, then the servers deleted in the
// last days while their copy is kept (docs/features/backups-page.md).
import { useSearchParams } from 'react-router';
import getAccountBackups from '@/api/me/backups/getAccountBackups.ts';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AccountBackupRow from './AccountBackupRow.tsx';

const page = 'pages.account.backups';

export default function DashboardBackups() {
  const { t } = useTranslations();
  const [searchParams] = useSearchParams();

  // ?server=<short id>, from the server's Backups page: that row is highlighted and scrolled to.
  const focused = (searchParams.get('server') ?? '').toLowerCase().replace(/[^0-9a-f]/g, '');

  const backups = useResource({
    queryKey: queryKeys.user.backups(),
    queryFn: getAccountBackups,
  });

  return (
    <AccountContentContainer title={t(`${page}.heading`, {})} subtitle={t(`${page}.subtitle`, {})}>
      <ResourceView resource={backups}>
        {(rows) => {
          const live = rows.filter((row) => !row.deleted);
          const deleted = rows.filter((row) => row.deleted);

          return (
            <div className='theme-backups'>
              <section className='theme-backups-section'>
                <h2 className='theme-eco-heading'>{t(`${page}.yourServers`, {})}</h2>

                {live.length > 0 ? (
                  <div className='theme-backups-list'>
                    {live.map((row) => (
                      <AccountBackupRow
                        key={row.serverUuid}
                        row={row}
                        focused={!!focused && row.serverUuidShort === focused}
                      />
                    ))}
                  </div>
                ) : (
                  <div className='theme-eco-card'>
                    <p className='theme-backups-text'>{t(`${page}.noServers`, {})}</p>
                  </div>
                )}
              </section>

              {/* Deleted in the last days: the last copy stays until the hold ends. */}
              {deleted.length > 0 && (
                <section className='theme-backups-section'>
                  <div className='theme-backups-section-head'>
                    <h2 className='theme-eco-heading'>{t(`${page}.deletedTitle`, {})}</h2>
                    <p className='theme-backups-text'>{t(`${page}.deletedText`, {})}</p>
                  </div>

                  <div className='theme-backups-list'>
                    {deleted.map((row) => (
                      <AccountBackupRow
                        key={row.serverUuid}
                        row={row}
                        focused={!!focused && row.serverUuidShort === focused}
                      />
                    ))}
                  </div>
                </section>
              )}
            </div>
          );
        }}
      </ResourceView>
    </AccountContentContainer>
  );
}
