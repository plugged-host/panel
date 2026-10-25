// plugged.host: the pieces both faces of the Transfer page share: a flag, the move's timeline
// ("Activity") and the earlier moves (docs/features/transfers.md)
import { z } from 'zod';
import Badge from '@/elements/data-display/Badge.tsx';
import { transferEventText } from '@/lib/domain/transfer.ts';
import { formatDateTime } from '@/lib/format/time.ts';
import {
  serverTransferEventSchema,
  serverTransferPlaceSchema,
  serverTransferRequestSchema,
} from '@/lib/schemas/server/transfer.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Place = z.infer<typeof serverTransferPlaceSchema>;
type Request = z.infer<typeof serverTransferRequestSchema>;
type Event = z.infer<typeof serverTransferEventSchema>;

export function TransferFlag({ place, size }: { place: Place | null; size?: 'sm' | 'lg' }) {
  return (
    <span className='theme-transfer-flag' data-size={size}>
      {place?.flag && <img src={`/flags/${place.flag}.svg`} alt='' />}
    </span>
  );
}

export function TransferTimeline({ events }: { events: Event[] }) {
  const { t, language } = useTranslations();
  const lines = events.flatMap((event) => {
    const text = transferEventText(t, event);
    return text === null ? [] : [{ event, text }];
  });

  if (lines.length === 0) return null;

  return (
    <section className='theme-eco-card'>
      <h3 className='theme-eco-heading'>{t('pages.server.transfer.timeline', {})}</h3>
      <div className='theme-transfer-log'>
        {lines.map(({ event, text }, index) => (
          <div key={`${event.at.getTime()}-${index}`} className='theme-transfer-log-row'>
            <i className='theme-transfer-dot' data-level={event.level} aria-hidden />
            <span>{text}</span>
            <time className='font-mono' dateTime={event.at.toISOString()}>
              {event.at.toLocaleTimeString(language, { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
            </time>
          </div>
        ))}
      </div>
    </section>
  );
}

const historyColor = (status: Request['status']) =>
  status === 'completed' ? 'green' : status === 'failed' ? 'red' : 'gray';

export function TransferHistory({ history }: { history: Request[] }) {
  const { t } = useTranslations();

  if (history.length === 0) return null;

  return (
    <section className='theme-eco-card'>
      <h3 className='theme-eco-heading'>{t('pages.server.transfer.history', {})}</h3>
      <div className='theme-transfer-hist'>
        {history.map((past) => (
          <div key={past.uuid} className='theme-transfer-hist-row'>
            <TransferFlag place={past.fromLocation} size='sm' />
            <span className='theme-transfer-arrow' aria-hidden>
              →
            </span>
            <TransferFlag place={past.toLocation} size='sm' />
            <strong>{past.toLocation?.name ?? t('pages.server.transfer.unknownLocation', {})}</strong>
            <Badge color={historyColor(past.status)} size='sm'>
              {past.status === 'completed'
                ? t('pages.server.transfer.historyState.completed', {})
                : past.status === 'failed'
                  ? t('pages.server.transfer.historyState.failed', {})
                  : t('pages.server.transfer.historyState.cancelled', {})}
            </Badge>
            <time dateTime={(past.finished ?? past.created).toISOString()}>
              {formatDateTime(past.finished ?? past.created)}
            </time>
          </div>
        ))}
      </div>
    </section>
  );
}
