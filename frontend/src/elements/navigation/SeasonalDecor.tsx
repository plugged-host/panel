// plugged.host: the seasonal theme's decorations (lib/seasonalTheme.ts, docs/features/theme.md#seasonal-themes)
import { useGlobalStore } from '@/stores/global.ts';

/*
 * Flat drawings in three places: by the sidebar's logo, on a shelf over the sidebar's footer, and
 * at the end of the phone's top bar (where the sidebar is a drawer). They are pictures only:
 * hidden from screen readers, never clickable, and still unless the system allows motion
 * (app.css, "Seasonal themes"). Default draws nothing.
 */

type Slot = 'logo' | 'shelf' | 'bar';

function Pumpkin({ face = false, className }: { face?: boolean; className?: string }) {
  return (
    <svg className={className} viewBox='0 0 32 30' aria-hidden='true' focusable='false'>
      <path d='M15.2 7.5V4.2c0-1.6.9-2.9 2.4-3.4l.8 1.4c-.9.4-1.4 1.1-1.4 2.1v3.2z' fill='#65a30d' />
      <ellipse cx='10' cy='18.5' rx='8' ry='10.5' fill='#c2410c' />
      <ellipse cx='22' cy='18.5' rx='8' ry='10.5' fill='#c2410c' />
      <ellipse cx='16' cy='18.5' rx='8.5' ry='11' fill='#ea580c' />
      {face ? (
        <path
          d='M10.5 13.5l2.8 4.2H7.7zM21.5 13.5l2.8 4.2h-5.6zM8.5 21h15l-2 3.8-2.4-1.6-2.6 2-2.6-2-2.4 1.6z'
          fill='#fcd34d'
        />
      ) : (
        <path
          d='M16 8.2c-1.2 3-1.2 17.6 0 20.6M12 9c-2.6 3.4-2.6 15.6 0 19M20 9c2.6 3.4 2.6 15.6 0 19'
          stroke='#c2410c'
          strokeWidth='0.9'
          fill='none'
        />
      )}
    </svg>
  );
}

function Bat({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox='0 0 32 14' aria-hidden='true' focusable='false'>
      <path
        d='M16 3.2c-.8 0-1.4.5-1.7 1.2L13.4 2.6C11.7 4 9.4 4.4 7.2 3.7 5 3 2.5 3.8 0 7c1.8-.6 3.5-.4 4.8.8.9.8 1.4 2 1.5 3.2 1.3-1.2 3.2-1.5 4.8-.7 1.2.6 2 1.7 2.3 3l1.7-2.4c.4.3.6.4.9.4s.5-.1.9-.4l1.7 2.4c.3-1.3 1.1-2.4 2.3-3 1.6-.8 3.5-.5 4.8.7.1-1.2.6-2.4 1.5-3.2 1.3-1.2 3-1.4 4.8-.8-2.5-3.2-5-4-7.2-3.3-2.2.7-4.5.3-6.2-1.1l-.9 1.8c-.3-.7-.9-1.2-1.7-1.2z'
        fill='#574a6e'
      />
      <circle cx='15.1' cy='5.6' r='0.55' fill='#fb923c' />
      <circle cx='16.9' cy='5.6' r='0.55' fill='#fb923c' />
    </svg>
  );
}

function Snowflake({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox='0 0 24 24' aria-hidden='true' focusable='false'>
      <path
        d='M12 2v20M3.3 7l17.4 10M3.3 17l17.4-10M9.6 3.8 12 5.6l2.4-1.8M9.6 20.2 12 18.4l2.4 1.8M4 10.6l2.8-.5-.6-2.9M20 13.4l-2.8.5.6 2.9M4 13.4l2.8.5-.6 2.9M20 10.6l-2.8-.5.6-2.9'
        stroke='#e0f2fe'
        strokeWidth='1.3'
        strokeLinecap='round'
        strokeLinejoin='round'
        fill='none'
      />
    </svg>
  );
}

function Tree({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox='0 0 24 30' aria-hidden='true' focusable='false'>
      <path d='M10.5 25h3v5h-3z' fill='#78350f' />
      <path d='M12 3 4.5 13h3L3 20h4L1.5 26h21L17 20h4l-4.5-7h3z' fill='#15803d' />
      <path d='M12 .2l.9 1.9 2.1.3-1.5 1.4.4 2.1-1.9-1-1.9 1 .4-2.1L9 2.4l2.1-.3z' fill='#facc15' />
      <circle cx='10' cy='11.5' r='1' fill='#dc2626' />
      <circle cx='14.5' cy='17' r='1' fill='#facc15' />
      <circle cx='8.5' cy='21.5' r='1' fill='#dc2626' />
      <circle cx='16' cy='23' r='1' fill='#e0f2fe' />
    </svg>
  );
}

function Present({ box, ribbon, className }: { box: string; ribbon: string; className?: string }) {
  return (
    <svg className={className} viewBox='0 0 20 20' aria-hidden='true' focusable='false'>
      <path d='M2 7h16v13H2z' fill={box} />
      <path d='M1 5h18v4H1z' fill={box} />
      <path d='M8.5 5h3v15h-3zM1 6h18v2H1z' fill={ribbon} />
      <path d='M10 5C8.5 2 5.5 1.5 5.5 3.3 5.5 5 8.5 5 10 5c1.5 0 4.5 0 4.5-1.7 0-1.8-3-1.3-4.5 1.7z' fill={ribbon} />
    </svg>
  );
}

function Halloween({ slot }: { slot: Slot }) {
  switch (slot) {
    case 'logo':
      return (
        <>
          <Bat className='theme-season-bat theme-season-logo-start' />
          <Bat className='theme-season-bat theme-season-bat-second theme-season-logo-start-second' />
          <Pumpkin face className='theme-season-logo-end' />
        </>
      );
    case 'shelf':
      return (
        <>
          <Pumpkin className='theme-season-small' />
          <Pumpkin face />
          <Pumpkin className='theme-season-small' />
        </>
      );
    case 'bar':
      return <Pumpkin face />;
  }
}

function Christmas({ slot }: { slot: Slot }) {
  switch (slot) {
    case 'logo':
      return (
        <>
          <Snowflake className='theme-season-flake theme-season-logo-start' />
          <Snowflake className='theme-season-flake theme-season-small theme-season-logo-start-second' />
          <Snowflake className='theme-season-flake theme-season-logo-end' />
        </>
      );
    case 'shelf':
      return (
        <>
          <Present box='#b91c1c' ribbon='#fde68a' className='theme-season-small' />
          <Tree />
          <Present box='#15803d' ribbon='#fecaca' className='theme-season-small' />
        </>
      );
    case 'bar':
      return <Tree />;
  }
}

function Gta6Vi({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox='0 0 1000 800' aria-hidden='true' focusable='false'>
      <defs>
        <linearGradient id='theme-gta6-sunset' x1='0' y1='0' x2='0' y2='1'>
          <stop offset='0%' stopColor='#2B60DA' />
          <stop offset='48%' stopColor='#FF4CB9' />
          <stop offset='100%' stopColor='#E6953E' />
        </linearGradient>
      </defs>
      <path
        d='M954.37,63.64H755.96v672.73h198.42V63.64z M738.15,28.01H990v743.98H720.33V42.93L571.04,352.15L368.77,771.07L10,28.01h274.16l84.61,171.97l84.61-171.97H720.33h7.2h10.61z M368.77,689.33L670.87,63.64H475.45L368.77,280.49L262.08,63.64H66.67L368.77,689.33z'
        fill='#12091f'
      />
      <polygon
        points='368.77,730.2 555.03,344.42 699.2,45.82 464.41,45.82 368.77,240.24 273.12,45.82 38.33,45.82 182.5,344.42'
        fill='url(#theme-gta6-sunset)'
      />
      <rect x='738.15' y='45.82' width='234.04' height='708.35' fill='url(#theme-gta6-sunset)' />
      <path
        d='M309.11,365.25c-2.77-10.64-4.9-21.64-6.65-32.85c1.45,15.19,1.21,30.31,0.85,45.41C305.22,373.62,307.15,369.43,309.11,365.25 M496.9,287.59l71.53,8.18l-48.52-51.46l22.16,38.62l-24.04-20.46l7.95,19.62l-42.45-34.65l29.69,39.03l-34.92-26.72L496.9,287.59z M866.18,257.57c0.71,9.44,1.63,19.15,2.72,29.1l-4.94-10.44C864.76,270.01,866.02,263.83,866.18,257.57z M813.92,242.54c-5.24-5.3-10.47-10.57-15.55-15.87c11.13,5.76,22.26,9.91,33.39,10.93C826.94,240.72,820.96,242.22,813.92,242.54z M882.8,290.52c3.33-10.25,7.34-20.38,12.03-30.3c-13.74,6.89-16.32,10.47-18.84,30.38C878.4,290.53,880.7,290.49,882.8,290.52z M923.62,280.49c5.19-2.22,10.58-4.36,16.18-6.41c-8.91-1.42-18.65-3.97-29.24-7.72C915.58,271.01,919.92,275.73,923.62,280.49z M895.51,294.58c6.57-3.88,13.66-7.56,21.24-11.04c-4.89-2.38-10.21-5.24-16.02-8.62l-13.17,15.85c0.98,0.1,1.88,0.25,2.68,0.44C892.79,291.81,894.36,292.9,895.51,294.58z'
        fill='#12091f'
      />
    </svg>
  );
}

function PalmTree({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox='0 0 32 34' aria-hidden='true' focusable='false'>
      <path
        d='M14 33c1.6-6.5 1.4-13.8-2-20.5 1.3.4 2.4.9 3.2 2 2.5 5.5 1.7 12.5.8 18.5z'
        fill='#1a0c28'
        stroke='#ff4cb9'
        strokeWidth='0.4'
      />
      <path d='M12 12.5C7.5 13 3.5 16.5 1.5 21c3-2.2 7-2.8 10.8-2-.8-2-.3-4.5.7-6.5z' fill='#160822' />
      <path d='M12 12.5C8 9.5 3.5 9 0.5 10.5c3.5.5 6.8 2.5 9.2 5.5.7-1.4 1.5-2.5 2.3-3.5z' fill='#220b33' />
      <path d='M12 12.5C11.5 7 12.5 3 15 0.8c.2 3.8-.5 7.2-3 11.7z' fill='#1b0829' />
      <path d='M12 12.5C16.5 9 21.5 8.5 26.5 10c-3.5 1-6.5 3.2-8.5 6.5-.5-1.5-2.2-2.8-6-4z' fill='#220b33' />
      <path d='M12 12.5C17 13.5 22.5 16.5 25.5 21.5c-3.2-2-7.5-2.6-11-1.5.8-2.5 0-5.5-2.5-7.5z' fill='#160822' />
      <path
        d='M12 12.5C9 13.5 5 16 3 19M12 12.5C9 10.5 5 10 2 10.5M12 12.5C12 7.5 13 4 14.5 2M12 12.5C16 10.5 20 10 24 11M12 12.5C16 14 20 16 23 19.5'
        stroke='#ff4cb9'
        strokeWidth='0.5'
        strokeLinecap='round'
        fill='none'
      />
    </svg>
  );
}

function Gta6({ slot }: { slot: Slot }) {
  switch (slot) {
    case 'logo':
      return (
        <>
          <PalmTree className='theme-season-palm theme-season-logo-start' />
          <PalmTree className='theme-season-palm theme-season-palm-second theme-season-small theme-season-logo-start-second' />
          <Gta6Vi className='theme-season-logo-end' />
        </>
      );
    case 'shelf':
      return (
        <>
          <PalmTree className='theme-season-small theme-season-palm-second' />
          <Gta6Vi className='theme-season-small' />
          <PalmTree className='theme-season-palm' />
        </>
      );
    case 'bar':
      return <Gta6Vi />;
  }
}

export default function SeasonalDecor({ slot }: { slot: Slot }) {
  const season = useGlobalStore((state) => state.settings.app?.seasonalTheme);

  if (season !== 'halloween' && season !== 'christmas' && season !== 'gta6') {
    return null;
  }

  return (
    <span className={`theme-season theme-season-${slot}`} aria-hidden='true'>
      {season === 'halloween' ? (
        <Halloween slot={slot} />
      ) : season === 'christmas' ? (
        <Christmas slot={slot} />
      ) : (
        <Gta6 slot={slot} />
      )}
    </span>
  );
}
