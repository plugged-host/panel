// plugged.host: a Minecraft server's worlds, laid out like the Pelican panel's Worlds page: one card
// per world, the loaded one first, and three notes on how it works underneath
// (docs/features/minecraft.md)
import { faArrowsRotate, faStop } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useEffect, useRef, useState } from 'react';
import { z } from 'zod';
import { useShallow } from 'zustand/react/shallow';
import getWorlds from '@/api/server/minecraft/getWorlds.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import { ServerCan } from '@/elements/Can.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftWorldsSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { usePowerAction } from '@/plugins/server/usePowerAction.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useReplacedToast } from './useReplacedToast.ts';
import WorldCard from './WorldCard.tsx';

const base = 'pages.server.minecraft.worlds';
const HELP = ['upload', 'safe', 'switch'] as const;

function Worlds({ worlds, offline }: { worlds: z.infer<typeof minecraftWorldsSchema>; offline: boolean }) {
  const { t } = useTranslations();
  const importing = worlds.import?.state === 'running';
  // Why the loaded world cannot be replaced now, if it cannot.
  const locked = !offline ? t(`${base}.stopFirst`, {}) : importing ? t(`${base}.importing`, {}) : null;

  return (
    <div className='flex flex-col gap-8'>
      {!offline && (
        <Notification className='theme-callout' color='yellow'>
          {t(`${base}.running`, {})}
        </Notification>
      )}
      {importing && (
        <Notification className='theme-callout' loading>
          {t(`${base}.importing`, {})}
        </Notification>
      )}
      {worlds.import?.state === 'failed' && (
        <Notification className='theme-callout' color='red'>
          {t(`${base}.importFailed`, { error: worlds.import.error ?? '' })}
        </Notification>
      )}

      <div className='theme-world-list'>
        {worlds.worlds.map((world) => (
          <WorldCard key={world.name} world={world} worlds={worlds} locked={locked} offline={offline} />
        ))}
      </div>

      <div className='theme-world-help'>
        {HELP.map((help) => (
          <div key={help}>
            <strong>{t(`${base}.help.${help}Title`, {})}</strong>
            <p>{t(`${base}.help.${help}Body`, {})}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function ServerMinecraftWorlds() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const replacedToast = useReplacedToast();
  const powerAction = usePowerAction();
  const { server, state, socketConnected } = useServerStore(
    useShallow((store) => ({ server: store.server, state: store.state, socketConnected: store.socketConnected })),
  );
  const [rescanning, setRescanning] = useState(false);

  const worlds = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.worlds(),
    queryFn: () => getWorlds(server.uuid),
  });

  // The console's live state once its socket is up (it says "offline" until then), else the
  // panel's answer with the list.
  const offline = socketConnected ? state === 'offline' : !(worlds.data?.running ?? true);

  // An import runs after its request: poll until it is done, then say how it ended.
  const importState = worlds.data?.import?.state;
  const previousState = useRef(importState);
  useEffect(() => {
    if (previousState.current === 'running' && importState === 'done' && worlds.data) {
      replacedToast(
        t(`${base}.toast.imported`, { world: worlds.data.generation.name }),
        worlds.data.import?.kept ?? null,
      );
    }
    previousState.current = importState;

    if (importState !== 'running') return;
    const timer = setInterval(() => worlds.refetch(), 3000);
    return () => clearInterval(timer);
  }, [importState]);

  const rescan = () => {
    setRescanning(true);
    worlds.refetch().finally(() => setRescanning(false));
  };

  const stop = () => {
    powerAction('stop');
    addToast(t(`${base}.toast.stopping`, {}), 'info');
  };

  return (
    <ServerContentContainer
      title={t(`${base}.title`, {})}
      subtitle={t(`${base}.subtitle`, {})}
      contentRight={
        <>
          {!offline && (
            <ServerCan action='control.stop'>
              <Tooltip label={t(`${base}.button.stop`, {})}>
                <ActionIcon size='input-sm' variant='subtle' color='red' onClick={stop}>
                  <FontAwesomeIcon icon={faStop} />
                </ActionIcon>
              </Tooltip>
            </ServerCan>
          )}
          <Tooltip label={t(`${base}.button.refresh`, {})}>
            <ActionIcon size='input-sm' variant='subtle' color='gray' loading={rescanning} onClick={rescan}>
              <FontAwesomeIcon icon={faArrowsRotate} />
            </ActionIcon>
          </Tooltip>
        </>
      }
    >
      <ResourceView resource={worlds}>{(data) => <Worlds worlds={data} offline={offline} />}</ResourceView>
    </ServerContentContainer>
  );
}
