// plugged.host: the game's heart and food icons, drawn here as pixel art so the bars need no images
// (docs/features/minecraft.md#player-details)

// 9×8 pixels: `X` the heart, `M` meat, `B` bone
const HEART = ['.XX...XX.', 'XXXX.XXXX', 'XXXXXXXXX', 'XXXXXXXXX', '.XXXXXXX.', '..XXXXX..', '...XXX...', '....X....'];
const SHANK = ['.....MMM.', '....MMMMM', '....MMMMM', '...MMMMMM', '...MMMMM.', '..BB.MM..', '.BB......', 'BB.......'];

type Fill = 'full' | 'half' | 'empty';

/** The pixels for which `take` holds, as one path of 1-pixel-high runs. */
function path(rows: string[], take: (cell: string, x: number) => boolean): string {
  let d = '';
  rows.forEach((row, y) => {
    let start = -1;
    for (let x = 0; x <= row.length; x++) {
      const on = x < row.length && row[x] !== '.' && take(row[x], x);
      if (on && start < 0) start = x;
      if (!on && start >= 0) {
        d += `M${start} ${y}h${x - start}v1h${start - x}z`;
        start = -1;
      }
    }
  });
  return d;
}

/** Left of this column a half icon is filled. */
const HALF = 5;

function PixelIcon({ rows, fill, colour }: { rows: string[]; fill: Fill; colour: (cell: string) => string }) {
  const filled = (x: number) => fill === 'full' || (fill === 'half' && x < HALF);
  const kinds = [...new Set(rows.join('').replaceAll('.', ''))];

  return (
    <svg className='theme-player-pixel' viewBox='0 0 9 8' aria-hidden>
      {kinds.map((kind) => (
        <path key={kind} className={colour(kind)} d={path(rows, (cell, x) => cell === kind && filled(x))} />
      ))}
      <path className='theme-player-pixel-empty' d={path(rows, (_cell, x) => !filled(x))} />
    </svg>
  );
}

/** One icon per two points, as the game draws health and food: full, half or empty. */
function fills(value: number, max: number): Fill[] {
  const points = Math.max(0, Math.ceil(value));
  return Array.from({ length: Math.ceil(max / 2) }, (_, i) => {
    const left = points - i * 2;
    return left >= 2 ? 'full' : left === 1 ? 'half' : 'empty';
  });
}

export function Hearts({ health, max }: { health: number; max: number }) {
  // a heart is two points; more than 50 hearts (a modded maximum) would only fill the page
  return (
    <div className='theme-player-pixels'>
      {fills(health, Math.min(max, 100)).map((fill, i) => (
        <PixelIcon key={i} rows={HEART} fill={fill} colour={() => 'theme-player-pixel-heart'} />
      ))}
    </div>
  );
}

export function Food({ food }: { food: number }) {
  return (
    <div className='theme-player-pixels'>
      {fills(food, 20).map((fill, i) => (
        <PixelIcon
          key={i}
          rows={SHANK}
          fill={fill}
          colour={(cell) => (cell === 'B' ? 'theme-player-pixel-bone' : 'theme-player-pixel-meat')}
        />
      ))}
    </div>
  );
}
