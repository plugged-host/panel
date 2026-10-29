// plugged.host: whether this account sees ads here, and how (docs/features/ads.md)
import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import getAccountAds from '@/api/me/ads/getAccountAds.ts';
import getPromoClips from '@/api/me/ads/getPromoClips.ts';
import { type PromoRotation, shuffled } from '@/lib/ads.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export type AdArea = 'account' | 'server';

export function useAccountAds() {
  const { user } = useAuth();
  const enabled = useGlobalStore((state) => !!state.settings.ads?.enabled);

  return useQuery({
    queryKey: queryKeys.user.ads(),
    queryFn: getAccountAds,
    enabled: enabled && !!user,
    staleTime: 5 * 60_000,
  });
}

export function useAds(area: AdArea) {
  const settings = useGlobalStore((state) => state.settings.ads);
  const { data: account } = useAccountAds();

  const shown =
    !!settings?.enabled &&
    (area === 'account' ? settings.showAccount : settings.showServer) &&
    !!account &&
    account.audience !== 'none';
  // the first membership sees only the waiting room before a start: no banners or popups on pages
  const pageShown = shown && account.audience !== 'room';

  return {
    settings,
    account,
    shown,
    pageShown,
    refreshSeconds:
      account?.audience === 'member' || account?.audience === 'room'
        ? settings.refreshMemberSeconds
        : settings?.refreshSeconds,
  };
}

// a new draw of the promo clips on every visit (page load), the same while it lasts
const VISIT = Math.random().toString(36).slice(2);

/**
 * The house promo clips the slots of this page may show (docs/features/promos.md#promo-clips), to
 * the same audience as every other unit: never to someone without ads, never the first
 * membership's clip to someone who has a membership, and none that leads to a page that is off.
 */
export function usePromoRotation(pathname: string): PromoRotation | undefined {
  const ads = useGlobalStore((state) => state.settings.ads);
  const economy = useGlobalStore((state) => state.settings.economy);
  const { data: account } = useAccountAds();
  const enabled = !!ads?.enabled && !!ads.promoVideos && !!account && account.audience !== 'none';

  const { data: clips } = useQuery({
    queryKey: ['promo', 'clips'],
    queryFn: getPromoClips,
    enabled,
    staleTime: Infinity,
    retry: false,
  });

  return useMemo(() => {
    if (!enabled || !clips?.length) return undefined;

    const shown = clips.filter((clip) => {
      if (clip.name === 'go-plus' && account?.audience !== 'everyone') return false;
      if (clip.href.startsWith('/account/membership')) return !!economy?.membershipEnabled;
      if (clip.href.startsWith('/account/tokens')) return !!economy?.tokensEnabled;
      return true;
    });
    if (shown.length === 0) return undefined;

    const seed = `${VISIT}:${pathname}`;
    return { clips: shuffled(shown, seed), share: ads.promoVideoShare, seed };
  }, [
    enabled,
    clips,
    account?.audience,
    economy?.membershipEnabled,
    economy?.tokensEnabled,
    ads?.promoVideoShare,
    pathname,
  ]);
}
