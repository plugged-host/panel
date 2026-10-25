// plugged.host: the toast after the loaded world was replaced (docs/features/minecraft.md)
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

/** Says what happened, then where the old world went or that a start generates the new one. */
export function useReplacedToast() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const base = 'pages.server.minecraft.worlds.toast';

  return (headline: string, kept: string | null) =>
    addToast(`${headline} ${kept ? t(`${base}.kept`, { name: kept }) : t(`${base}.startToPlay`, {})}`, 'success');
}
