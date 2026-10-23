// plugged.host: one server on the account's Backups page (docs/features/backups-page.md).
import { faFileArrowDown } from '@fortawesome/free-solid-svg-icons';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import downloadAccountBackup from '@/api/me/backups/downloadAccountBackup.ts';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { bytesToString } from '@/lib/format/size.ts';
import { downloadUrl } from '@/lib/network/url.ts';
import { userAccountBackupSchema } from '@/lib/schemas/user/backups.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function AccountBackupRow({ row }: { row: z.infer<typeof userAccountBackupSchema> }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const page = 'pages.account.backups';
  const backup = row.backup;

  const doDownload = () => {
    if (!backup) return;

    downloadAccountBackup(backup.uuid)
      .then(({ url }) => {
        addToast(t('pages.server.backups.toast.downloadStarted', {}), 'success');
        downloadUrl(url);
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      });
  };

  return (
    <ContextMenu
      items={[
        {
          type: 'action',
          icon: faFileArrowDown,
          label: t('common.button.download', {}),
          hidden: !backup,
          onClick: doDownload,
          color: 'gray',
        },
      ]}
    >
      {({ items, openMenu }) => (
        <TableRow
          onContextMenu={(e) => {
            e.preventDefault();
            openMenu(e.clientX, e.clientY);
          }}
        >
          <TableData>
            {row.serverName}
            {row.deleted && (
              <Text size='xs' c='dimmed'>
                {t(`${page}.deleted`, {})} <FormattedTimestamp timestamp={row.deleted} />
              </Text>
            )}
          </TableData>
          <TableData>
            {backup ? (
              <FormattedTimestamp timestamp={backup.created} />
            ) : (
              <Text size='sm' c='dimmed'>
                {t(`${page}.noCopy`, {})}
              </Text>
            )}
          </TableData>
          <TableData>{backup ? bytesToString(backup.bytes) : null}</TableData>
          <TableData>{row.keptUntil ? <FormattedTimestamp timestamp={row.keptUntil} /> : null}</TableData>

          <ContextMenuToggle items={items} openMenu={openMenu} />
        </TableRow>
      )}
    </ContextMenu>
  );
}
