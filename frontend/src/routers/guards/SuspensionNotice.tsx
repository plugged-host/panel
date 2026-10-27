// plugged.host: what the owner and subusers of a suspended server read (docs/features/suspension.md).
import { Button as MantineButton } from '@mantine/core';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { formatDateTime } from '@/lib/format/time.ts';
import { intlLocale } from '@/lib/languages.ts';
import { serverSchema } from '@/lib/schemas/server/server.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

/**
 * The time left until `deleteAt` in words ("in 3 days", "in 5 hours", "now"). Rounded down, so it
 * never says more time is left than there is; the exact date is shown beside it.
 */
function deletionIn(deleteAt: Date, now: number, language: string): string {
  const seconds = Math.floor((deleteAt.getTime() - now) / 1000);
  const locale = intlLocale(language);

  // less than a minute left, or due: the purge deletes it on its next run (every 5 minutes)
  if (seconds < 60) {
    return new Intl.RelativeTimeFormat(locale, { numeric: 'auto' }).format(0, 'second');
  }

  // always a number: rounded-down days are not calendar days, so "tomorrow" could be wrong
  const format = new Intl.RelativeTimeFormat(locale, { numeric: 'always' });
  if (seconds < 3_600) {
    return format.format(Math.floor(seconds / 60), 'minute');
  }
  if (seconds < 172_800) {
    return format.format(Math.floor(seconds / 3_600), 'hour');
  }

  return format.format(Math.floor(seconds / 86_400), 'day');
}

export default function SuspensionNotice({ server }: { server: z.infer<typeof serverSchema> }) {
  const { t, language } = useTranslations();
  const suspension = server.suspension;

  // the time left counts down while the page stays open
  const deleteAt = suspension?.deleteAt?.getTime() ?? null;
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (deleteAt === null) return;

    const interval = setInterval(() => setNow(Date.now()), 60_000);

    return () => clearInterval(interval);
  }, [deleteAt]);

  if (!suspension) {
    return t('elements.screenBlock.serverConflict.contentSuspended', {});
  }

  return (
    <div className='flex flex-col gap-2'>
      <p>
        {suspension.suspended
          ? t(`elements.screenBlock.serverConflict.suspension.by.${suspension.kind}`, {
              date: formatDateTime(suspension.suspended),
            })
          : t('elements.screenBlock.serverConflict.contentSuspended', {})}
      </p>
      {suspension.reason && (
        <p className='text-(--mantine-color-text) whitespace-pre-line break-words'>
          {t('elements.screenBlock.serverConflict.suspension.reason', { reason: suspension.reason })}
        </p>
      )}
      {/* the purge's deletion time (suspension_delete_at); none: never deleted, nothing said */}
      {suspension.deleteAt && (
        <>
          <p className='text-(--mantine-color-text)'>
            {t('elements.screenBlock.serverConflict.suspension.deletesIn', {
              time: deletionIn(suspension.deleteAt, now, language),
              date: formatDateTime(suspension.deleteAt, false, false),
            })}
          </p>
          <p>{t('elements.screenBlock.serverConflict.suspension.notEarlier', {})}</p>
        </>
      )}
      {suspension.helpUrl ? (
        <>
          <p>{t('elements.screenBlock.serverConflict.suspension.helpLink', {})}</p>
          {/* neo's support link, else the Discord invite (the Pelican panel's "Get help" action) */}
          <div>
            <MantineButton
              component='a'
              href={suspension.helpUrl}
              target='_blank'
              rel='noopener noreferrer'
              variant='default'
              fw={600}
            >
              {t('elements.screenBlock.serverConflict.suspension.getHelp', {})}
            </MantineButton>
          </div>
        </>
      ) : (
        <p>{t('elements.screenBlock.serverConflict.suspension.help', {})}</p>
      )}
    </div>
  );
}
