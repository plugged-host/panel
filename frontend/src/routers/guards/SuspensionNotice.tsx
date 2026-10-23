// plugged.host: what the owner and subusers of a suspended server read (docs/features/suspension.md).
import { z } from 'zod';
import { formatDateTime } from '@/lib/format/time.ts';
import { serverSchema } from '@/lib/schemas/server/server.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function SuspensionNotice({ server }: { server: z.infer<typeof serverSchema> }) {
  const { t } = useTranslations();
  const suspension = server.suspension;

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
      {suspension.deleteAt && (
        <p>
          {t('elements.screenBlock.serverConflict.suspension.deleteAt', {
            date: formatDateTime(suspension.deleteAt),
          })}
        </p>
      )}
      <p>{t('elements.screenBlock.serverConflict.suspension.help', {})}</p>
    </div>
  );
}
