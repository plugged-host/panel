import { useState } from 'react';
import { useNavigate } from 'react-router'; // plugged.host
import { z } from 'zod';
import sendPowerAction from '@/api/server/sendPowerAction.ts';
import { serverPowerAction } from '@/lib/schemas/server/server.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export function useBulkPowerActions() {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate(); // plugged.host
  const [bulkActionLoading, setBulkActionLoading] = useState<z.infer<typeof serverPowerAction> | null>(null);

  const handleBulkPowerAction = async (serverUuids: string[], action: z.infer<typeof serverPowerAction>) => {
    setBulkActionLoading(action);

    const results = await Promise.allSettled(serverUuids.map((uuid) => sendPowerAction(uuid, action)));

    // plugged.host: a start a gate holds (shared/src/start_gates.rs) did not happen yet. One server goes to
    // the gate's page; several are counted apart.
    const held = results.filter((r) => r.status === 'fulfilled' && r.value.url).length;
    const successful = results.filter((r) => r.status === 'fulfilled').length - held;
    const failed = results.filter((r) => r.status === 'rejected').length;

    if (serverUuids.length === 1 && results[0].status === 'fulfilled' && results[0].value.url) {
      const url = results[0].value.url;
      setBulkActionLoading(null);
      if (url.startsWith('/')) {
        navigate(url);
      } else {
        window.location.assign(url);
      }
      return;
    }

    if (held > 0) {
      addToast(t('pages.account.home.bulkActions.held', { servers: tItem('server', held) }), 'info');
    }

    const actionPastTenseMap: Record<
      z.infer<typeof serverPowerAction>,
      'started' | 'stopped' | 'restarted' | 'killed'
    > = {
      start: 'started',
      stop: 'stopped',
      restart: 'restarted',
      kill: 'killed',
    };
    const actionPastTense = actionPastTenseMap[action];

    if (successful === 0 && failed === 0) {
      // plugged.host: every start was held, which the toast above says.
      setBulkActionLoading(null);
      return;
    }

    if (failed === 0) {
      addToast(
        t('pages.account.home.bulkActions.success', {
          servers: tItem('server', successful),
          action: t(`common.enum.bulkActionServerAction.${actionPastTense}`, {}),
        }),
        'success',
      );
    } else {
      addToast(
        t('pages.account.home.bulkActions.partial', {
          successfulServers: tItem('server', successful),
          failedServers: tItem('server', failed),
          action: t(`common.enum.bulkActionServerAction.${actionPastTense}`, {}),
        }),
        'warning',
      );
    }

    setBulkActionLoading(null);
  };

  return { handleBulkPowerAction, bulkActionLoading };
}
