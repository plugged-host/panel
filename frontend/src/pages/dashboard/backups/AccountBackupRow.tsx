// plugged.host: one server on the account's Backups page, a row of the Pelican panel's backups
// portal: its name, "egg · node · #id", the state of its newest copy, and Download, Manage and
// Backups at the end of the row (docs/features/backups-page.md).
import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router';
import { ClipboardCancelledError } from 'shared';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import downloadAccountBackup from '@/api/me/backups/downloadAccountBackup.ts';
import Button from '@/elements/buttons/Button.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import { copyToClipboard } from '@/lib/clipboard/copy.ts';
import { bytesToString } from '@/lib/format/size.ts';
import { formatTimestamp } from '@/lib/format/time.ts';
import { intlLocale } from '@/lib/languages.ts';
import { downloadUrl } from '@/lib/network/url.ts';
import { userAccountBackupSchema } from '@/lib/schemas/user/backups.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const page = 'pages.account.backups';

/** After this long "Starting…" becomes "Downloading, see your browser", as on the portal. */
const DOWNLOADING_AFTER_MS = 2_500;
/** A second click within this time does nothing, so it does not start a second download. */
const DOWNLOAD_BLOCKED_MS = 30_000;

type DownloadState = 'idle' | 'starting' | 'downloading';

function Sep() {
  return (
    <span className='theme-backups-sep' aria-hidden>
      ·
    </span>
  );
}

export default function AccountBackupRow({
  row,
  focused,
}: {
  row: z.infer<typeof userAccountBackupSchema>;
  focused: boolean;
}) {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const backup = row.backup;

  const ref = useRef<HTMLDivElement>(null);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const [download, setDownload] = useState<DownloadState>('idle');

  // Linked from the server's Backups page: bring the row into view once it is there.
  useEffect(() => {
    if (focused) ref.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });
  }, [focused]);

  useEffect(() => {
    const pending = timers.current;
    return () => pending.forEach(clearTimeout);
  }, []);

  const doDownload = () => {
    if (!backup || download !== 'idle') return;

    setDownload('starting');
    downloadAccountBackup(backup.uuid)
      .then(({ url }) => {
        downloadUrl(url);
        timers.current.push(
          setTimeout(() => setDownload('downloading'), DOWNLOADING_AFTER_MS),
          setTimeout(() => setDownload('idle'), DOWNLOAD_BLOCKED_MS),
        );
      })
      .catch((msg) => {
        setDownload('idle');
        addToast(httpErrorToHuman(msg), 'error');
      });
  };

  const copyId = () => {
    copyToClipboard(row.serverUuidShort)
      .then(() => addToast(t(`${page}.idCopied`, { id: row.serverUuidShort }), 'success'))
      .catch((err) => {
        if (err instanceof ClipboardCancelledError) return;
        addToast(t('elements.copyOnClick.toast.failed', {}), 'error');
      });
  };

  const size = backup && backup.bytes > 0 ? bytesToString(backup.bytes, 1) : null;
  const sizePart = size && (
    <>
      <Sep />
      <span>{size}</span>
    </>
  );

  return (
    <div ref={ref} className='theme-backups-row' data-focused={focused || undefined}>
      <div className='theme-backups-main'>
        <div className='theme-backups-name' title={row.serverName}>
          {row.serverName}
        </div>

        <div className='theme-backups-meta'>
          {row.eggName && (
            <>
              <span>{row.eggName}</span>
              <Sep />
            </>
          )}
          <span>{row.nodeName}</span>
          <Sep />
          <button type='button' className='theme-backups-id' onClick={copyId} title={t(`${page}.copyId`, {})}>
            #{row.serverUuidShort}
          </button>
        </div>

        <div className='theme-backups-status'>
          {!backup ? (
            <>
              <span className='theme-backups-dot' aria-hidden />
              <span>{t(`${page}.waiting`, {})}</span>
            </>
          ) : row.deleted ? (
            <>
              <span className='theme-backups-dot' data-ok aria-hidden />
              <span>{t(`${page}.deletedAgo`, { ago: formatTimestamp(row.deleted) })}</span>
              {row.keptUntil && (
                <>
                  <Sep />
                  <span>
                    {t(`${page}.keptUntil`, {
                      date: row.keptUntil.toLocaleDateString(intlLocale(language), { day: 'numeric', month: 'short' }),
                    })}
                  </span>
                </>
              )}
              {sizePart}
            </>
          ) : (
            <>
              <span className='theme-backups-dot' data-ok aria-hidden />
              <span>{t(`${page}.backupReady`, {})}</span>
              {sizePart}
              <Sep />
              <FormattedTimestamp timestamp={backup.created} />
            </>
          )}
        </div>
      </div>

      {(backup || !row.deleted) && (
        <div className='theme-backups-actions'>
          {backup && (
            <Button variant='default' aria-disabled={download !== 'idle' || undefined} onClick={doDownload}>
              {download === 'starting'
                ? t(`${page}.starting`, {})
                : download === 'downloading'
                  ? t(`${page}.downloading`, {})
                  : t('common.button.download', {})}
            </Button>
          )}

          {!row.deleted && (
            <>
              <Link
                to={`/server/${row.serverUuidShort}`}
                className='theme-backups-link'
                title={t(`${page}.manageHint`, {})}
              >
                {t(`${page}.manage`, {})}
              </Link>
              <Link
                to={`/server/${row.serverUuidShort}/backups`}
                className='theme-backups-link'
                title={t(`${page}.backupsHint`, {})}
              >
                {t(`${page}.backups`, {})}
              </Link>
            </>
          )}
        </div>
      )}
    </div>
  );
}
