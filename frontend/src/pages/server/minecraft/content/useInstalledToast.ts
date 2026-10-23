// plugged.host: the toast after a plugin or mod was installed (docs/features/minecraft.md)
import { z } from 'zod';
import { minecraftInstalledSchema } from '@/lib/schemas/server/minecraft.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

/** Tells where the file went and that a restart loads it. */
export function useInstalledToast() {
  const { t } = useTranslations();
  const { addToast } = useToast();

  return (installed: z.infer<typeof minecraftInstalledSchema>) =>
    addToast(
      t('pages.server.minecraft.content.browse.toast.installed', {
        file: installed.file,
        directory: installed.directory,
      }),
      'success',
    );
}
