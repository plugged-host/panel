// plugged.host: the viewed server's power actions over the API, so a start gate can hold a start
// and send the user to its page (shared/src/start_gates.rs). Stop and kill keep the websocket.
import { useCallback } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import sendPowerAction from '@/api/server/sendPowerAction.ts';
import { serverPowerAction } from '@/lib/schemas/server/server.ts';
import { SocketRequest } from '@/plugins/websocket/useWebsocketEvent.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export function usePowerAction() {
  const navigate = useNavigate();
  const { addToast } = useToast();
  const uuid = useServerStore((state) => state.server.uuid);
  const socketInstance = useServerStore((state) => state.socketInstance);

  return useCallback(
    async (action: z.infer<typeof serverPowerAction>) => {
      if ((action === 'stop' || action === 'kill') && socketInstance) {
        socketInstance.send(SocketRequest.SET_STATE, action);
        return;
      }

      try {
        const { url } = await sendPowerAction(uuid, action);

        if (url) {
          if (url.startsWith('/')) {
            navigate(url);
          } else {
            window.location.assign(url);
          }
        }
      } catch (error) {
        addToast(httpErrorToHuman(error), 'error');
      }
    },
    [uuid, socketInstance, navigate, addToast],
  );
}
