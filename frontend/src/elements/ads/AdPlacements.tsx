// plugged.host: the slots every page has (docs/features/ads.md#slots)
import { useLocation } from 'react-router';
import { serverPageKind } from '@/lib/ads.ts';
import { useGlobalStore } from '@/stores/global.ts';
import { AdSlot } from './AdSlot.tsx';

/** The sponsored top banner above the page (no ad zone since the Pelican panel's ads 2.2.2). */
export function AdTop() {
  const on = useGlobalStore((state) => !!state.settings.ads?.slotTop);
  if (!on) return null;

  return (
    <div className='theme-main theme-main-banner'>
      <AdSlot kind='top' />
    </div>
  );
}

/** A leaderboard and the native unit at the foot of the page (the Pelican panel's ads 2.2.2). */
export function AdFoot() {
  const on = useGlobalStore((state) => !!state.settings.ads?.slotFoot);
  if (!on) return null;

  return (
    <div className='theme-main theme-ad-foot'>
      <AdSlot kind='footLeaderboard' />
      <AdSlot kind='native' />
    </div>
  );
}

/** A strip under the header of a server's pages, but not the console's or a waiting room's. */
export function AdTools() {
  const { pathname } = useLocation();
  const on = useGlobalStore((state) => !!state.settings.ads?.slotServerTools);
  if (!on || serverPageKind(pathname) !== 'page') return null;

  return <AdSlot kind='tools' />;
}

/** A rectangle in a page's own content (Create Server, the Resource Store, the server list). */
export function AdPageSlot() {
  const on = useGlobalStore((state) => !!state.settings.ads?.slotPages);
  if (!on) return null;

  return <AdSlot kind='page' />;
}
