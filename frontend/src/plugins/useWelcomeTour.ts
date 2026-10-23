// plugged.host: the signed-in user's welcome tour and the calls that move it on
// (docs/features/welcome-tour.md)
import { useQueryClient } from '@tanstack/react-query';
import { useCallback } from 'react';
import { z } from 'zod';
import getWelcomeTour from '@/api/me/account/getWelcomeTour.ts';
import updateWelcomeTour from '@/api/me/account/updateWelcomeTour.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { welcomeTourSchema, welcomeTourUpdateSchema } from '@/lib/schemas/user/welcomeTour.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type Tour = z.infer<typeof welcomeTourSchema>;
type Update = z.infer<typeof welcomeTourUpdateSchema>;

export function useWelcomeTour() {
  const { user } = useAuth();
  const enabled = useGlobalStore((state) => !!state.settings.tour?.enabled);
  const queryClient = useQueryClient();

  const { data: tour } = useResource({
    queryKey: queryKeys.user.welcomeTour(),
    queryFn: getWelcomeTour,
    enabled: enabled && !!user && !user.suspended,
    silent: true,
  });

  // the step shows at once; the panel only keeps where the user got to
  const update = useCallback(
    (data: Update) => {
      queryClient.setQueryData<Tour>(queryKeys.user.welcomeTour(), (current) => {
        if (!current) return current;

        switch (data.action) {
          case 'advance':
            return { ...current, step: data.step ?? 1 };
          case 'advance_server':
            return { ...current, serverStep: data.step ?? 1 };
          case 'complete':
            return { ...current, panelActive: false, serverActive: true, step: 1 };
          case 'skip':
            return { ...current, panelActive: false, serverActive: false };
          case 'complete_server':
            return { ...current, serverActive: false, serverStep: 1 };
          case 'restart':
            return { panelActive: true, serverActive: false, step: 1, serverStep: 1 };
        }
      });

      return updateWelcomeTour(data)
        .then((saved) => queryClient.setQueryData(queryKeys.user.welcomeTour(), saved))
        .catch(() => queryClient.invalidateQueries({ queryKey: queryKeys.user.welcomeTour() }));
    },
    [queryClient],
  );

  return { enabled, tour: enabled ? tour : undefined, update };
}
