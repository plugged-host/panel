// plugged.host: what the spots that lead free players to Paid show: whether a plan is on sale, the product's name and the lowest price (docs/features/dedicated.md)
import { formatEuro } from '@/elements/AddFundsLink.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export default function usePaidOffer() {
  const { t, language } = useTranslations();
  const games = useGlobalStore((state) => state.settings.games);

  return {
    t,
    offered: !!games?.dedicatedOffered,
    name: games?.dedicatedName || 'Paid',
    /** The cheapest plan's price a month, formatted; null when no plan has one. */
    from: games?.dedicatedFrom != null ? formatEuro(games.dedicatedFrom, language) : null,
  };
}
