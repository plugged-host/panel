// plugged.host: what players call the location tiers: the memberships that open them
// (docs/features/economy.md)
import { useGlobalStore } from '@/stores/global.ts';

export function tierName(tier: string | null): string {
  const economy = useGlobalStore.getState().settings.economy;

  switch (tier) {
    case 'plus':
      return economy?.plusName ?? tier;
    case 'premium':
      return economy?.plusplusName ?? tier;
    default:
      return tier ?? '';
  }
}
