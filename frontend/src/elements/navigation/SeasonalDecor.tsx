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

export default function SeasonalDecor({ slot }: { slot: Slot }) {
  const season = useGlobalStore((state) => state.settings.app?.seasonalTheme);

  if (season !== 'halloween' && season !== 'christmas') {
    return null;
  }

  return (
    <span className={`theme-season theme-season-${slot}`} aria-hidden='true'>
      {season === 'halloween' ? <Halloween slot={slot} /> : <Christmas slot={slot} />}
    </span>
  );
}
