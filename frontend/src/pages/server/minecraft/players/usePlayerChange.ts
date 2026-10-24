// plugged.host: one change to the whitelist, the operators or the bans, with its toast
// (docs/features/minecraft.md)
import { useQueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import updatePlayer from '@/api/server/minecraft/updatePlayer.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftPlayerListSchema } from '@/lib/schemas/server/minecraft.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type List = z.infer<typeof minecraftPlayerListSchema>;

export function usePlayerChange() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.players.toast';

  const message = (list: List, add: boolean, name: string) => {
    switch (list) {
      case 'whitelist':
        return add ? t(`${base}.whitelisted`, { name }) : t(`${base}.unwhitelisted`, { name });
      case 'operators':
        return add ? t(`${base}.opped`, { name }) : t(`${base}.deopped`, { name });
      default:
        return add ? t(`${base}.banned`, { name }) : t(`${base}.pardoned`, { name });
    }
  };

  /** Resolves to whether it worked; errors are toasted. */
  return (list: List, add: boolean, name: string, reason?: string): Promise<boolean> =>
    updatePlayer(server.uuid, list, add, name, reason)
      .then((live) => {
        addToast(
          live ? message(list, add, name) : `${message(list, add, name)} ${t(`${base}.atNextStart`, {})}`,
          'success',
        );
        return true;
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
        return false;
      })
      .finally(() => queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.players() }));
}
