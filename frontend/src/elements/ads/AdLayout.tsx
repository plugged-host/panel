// plugged.host: the ads above and below a dashboard or server page (docs/features/ads.md)
import { useLocation } from 'react-router';
import { serverPageKind } from '@/lib/ads.ts';
import { type AdArea } from '@/plugins/useAds.ts';
import AdBlockNotice from './AdBlockNotice.tsx';
import { AdFoot, AdTop } from './AdPlacements.tsx';
import AdScripts from './AdScripts.tsx';

/**
 * Pages get the top and foot slots and the page-level scripts; the console no scripts; the
 * waiting rooms only their own scripts (their page shows the ads); a parked server nothing but
 * what its page shows.
 */
function useAdPageKind(area: AdArea, limbo: boolean) {
  const { pathname } = useLocation();
  return area === 'account' ? 'page' : limbo ? 'limbo' : serverPageKind(pathname);
}

/** Above the page: the ad-block notice, the top slot and the page-level scripts. */
export function AdLayoutTop({ area, limbo = false }: { area: AdArea; limbo?: boolean }) {
  const kind = useAdPageKind(area, limbo);

  return (
    <>
      {kind !== 'limbo' && <AdBlockNotice area={area} console={kind === 'console'} />}
      {(kind === 'page' || kind === 'console') && <AdTop />}
      <AdScripts area={area} kind={kind === 'page' ? 'page' : kind === 'room' ? 'room' : 'none'} />
    </>
  );
}

/** Below the page: the foot slots. */
export function AdLayoutFoot({ area, limbo = false }: { area: AdArea; limbo?: boolean }) {
  const kind = useAdPageKind(area, limbo);

  return kind === 'page' || kind === 'console' ? <AdFoot /> : null;
}
