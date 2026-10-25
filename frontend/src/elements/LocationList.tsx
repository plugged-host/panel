// plugged.host: locations to pick one from, as full-width rows like the Pelican panel's Transfer page
// (".pht-row"): flag, name, badges, latency and a radio (limbo's restore, docs/features/limbo.md). The
// Transfer page draws its rows with the same classes.
import { ReactNode } from 'react';
import { z } from 'zod';
import { serverCreateLocationSchema } from '@/lib/schemas/server/create.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Location = z.infer<typeof serverCreateLocationSchema>;

interface Props {
  locations: Location[];
  selected: string | null;
  onSelect: (uuid: string) => void;
  /** Badges after the name (a tier, "Full"). */
  badges?: (location: Location) => ReactNode;
  disabled?: (location: Location) => boolean;
}

export default function LocationList({ locations, selected, onSelect, badges, disabled }: Props) {
  const { t } = useTranslations();

  return (
    <div className='theme-location-list'>
      {locations.map((location) => (
        <button
          key={location.uuid}
          type='button'
          className='theme-location-row'
          data-selected={location.uuid === selected || undefined}
          aria-pressed={location.uuid === selected}
          disabled={disabled?.(location) ?? false}
          onClick={() => onSelect(location.uuid)}
        >
          <span className='theme-flag'>{location.flag && <img src={`/flags/${location.flag}.svg`} alt='' />}</span>
          <span className='theme-location-main'>
            {location.name}
            {badges?.(location)}
          </span>
          {location.latency !== null && (
            <span className='theme-location-ms font-mono'>
              {t('pages.account.createServer.latency', { ms: location.latency })}
            </span>
          )}
          <span className='theme-radio' aria-hidden />
        </button>
      ))}
    </div>
  );
}
