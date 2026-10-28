// plugged.host: whether this account sees ads here, and how (docs/features/ads.md)
import { useQuery } from '@tanstack/react-query';
import getAccountAds from '@/api/me/ads/getAccountAds.ts';
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
