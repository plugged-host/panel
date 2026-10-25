// plugged.host: one player of a Minecraft server: who they are and whether they are on, their health,
// inventory and position from the last save, their statistics, and what can be done to them: by
// console while the server runs and they play, in their files while it is stopped
// (docs/features/minecraft.md#player-details)
import { faArrowsRotate } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router';
import { z } from 'zod';
import { useShallow } from 'zustand/react/shallow';
import { httpErrorToHuman } from '@/api/axios.ts';
import getOnlinePlayers from '@/api/server/minecraft/getOnlinePlayers.ts';
import getPlayerDetails from '@/api/server/minecraft/getPlayerDetails.ts';
import setPlayerGameMode from '@/api/server/minecraft/setPlayerGameMode.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import Select from '@/elements/input/Select.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftGameModeSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import DeletePlayerData from './DeletePlayerData.tsx';
import PlayerAccess from './PlayerAccess.tsx';
import PlayerHead from './PlayerHead.tsx';
import PlayerInventory from './PlayerInventory.tsx';
import PlayerPosition from './PlayerPosition.tsx';
import PlayerStats from './PlayerStats.tsx';
import PlayerVitals from './PlayerVitals.tsx';
import { PlayerData, PlayerDetails, PlayerPage, playerBase } from './playerPage.ts';

type GameMode = z.infer<typeof minecraftGameModeSchema>;

const GAME_MODES: GameMode[] = ['survival', 'creative', 'adventure', 'spectator'];

export default function ServerMinecraftPlayer() {
  const { t, tReact } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const params = useParams<'uuid'>();
  const uuid = (params.uuid ?? '').toLowerCase();
  const { server, state, socketConnected } = useServerStore(
    useShallow((store) => ({ server: store.server, state: store.state, socketConnected: store.socketConnected })),
  );
  const canConsole = useServerCan('control.console');
  const canEdit = useServerCan('files.update');
  const canDelete = useServerCan('files.delete');
  const [reloading, setReloading] = useState(false);
  const [changingMode, setChangingMode] = useState(false);

  const key = queryKeys.server(server.uuid).minecraft.player(uuid);
  const details = useResource({
    queryKey: key,
    queryFn: () => getPlayerDetails(server.uuid, uuid),
    silent: true,
  });

  // The console's live state once its socket is up, else the panel's answer with the details.
  const running = socketConnected ? state === 'running' : (details.data?.running ?? false);
  const stopped = socketConnected ? state === 'offline' : (details.data?.stopped ?? false);

  // Asked once when the page opens and after a change, never on a timer: each ask types `list`.
  const online = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.online(),
    queryFn: () => getOnlinePlayers(server.uuid),
    enabled: running && canConsole,
    silent: true,
  });

  // A server that stops has just saved everyone: read the files again.
  const previousState = useRef(state);
  useEffect(() => {
    if (previousState.current !== state && (state === 'offline' || state === 'running')) {
      details.refetch();
    }
    previousState.current = state;
  }, [state]);

  const reload = () => {
    setReloading(true);
    Promise.all([details.refetch(), running && canConsole ? online.refetch() : Promise.resolve()]).finally(() =>
      setReloading(false),
    );
  };

  const gameModeName = (mode: GameMode) => {
    switch (mode) {
      case 'survival':
        return t(`${playerBase}.gameModes.survival`, {});
      case 'creative':
        return t(`${playerBase}.gameModes.creative`, {});
      case 'adventure':
        return t(`${playerBase}.gameModes.adventure`, {});
      default:
        return t(`${playerBase}.gameModes.spectator`, {});
    }
  };

  const page = (data: PlayerDetails): PlayerPage => {
    const name = data.name || data.uuid;

    // whether they are on: by UUID, or by name on servers whose `list` names only (before 1.13)
    let isOnline: boolean | null = false;
    if (running) {
      const players = online.data?.players;
      if (!players) {
        isOnline = null;
      } else if (players.some((player) => player.uuid?.toLowerCase() === data.uuid)) {
        isOnline = true;
      } else if (players.some((player) => !player.uuid)) {
        isOnline = data.name ? players.some((player) => player.name.toLowerCase() === data.name?.toLowerCase()) : null;
      }
    }

    const liveBlocked = !running
      ? t(`${playerBase}.reason.notRunning`, {})
      : !canConsole
        ? t(`${playerBase}.reason.noConsole`, {})
        : isOnline === false
          ? t(`${playerBase}.reason.notOnline`, {})
          : null;
    const changeBlocked = running
      ? liveBlocked
      : !stopped
        ? t(`${playerBase}.reason.notRunning`, {})
        : !canEdit
          ? t(`${playerBase}.reason.noFiles`, {})
          : null;

    return {
      details: data,
      name,
      running,
      stopped,
      online: isOnline,
      liveBlocked,
      changeBlocked,
      canEdit,
      canDelete,
      patch: (change: (current: PlayerData) => PlayerData) =>
        queryClient.setQueryData<PlayerDetails>(key, (current) =>
          current?.data ? { ...current, data: change(current.data) } : current,
        ),
      reload: () => details.refetch(),
      done: (message: string, live: boolean) =>
        addToast(live ? message : `${message} ${t(`${playerBase}.toast.nextJoin`, {})}`, 'success'),
    };
  };

  const changeGameMode = (current: PlayerPage, mode: GameMode) => {
    setChangingMode(true);
    setPlayerGameMode(server.uuid, current.details.uuid, mode)
      .then((live) => {
        current.done(t(`${playerBase}.toast.gameMode`, { name: current.name, mode: gameModeName(mode) }), live);
        if (live) {
          current.patch((data) => ({ ...data, gameMode: mode }));
        } else {
          current.reload();
        }
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setChangingMode(false));
  };

  const back = () => navigate(`/server/${server.uuidShort}/minecraft/players`);

  return (
    <ServerContentContainer
      title={details.data?.name || t('pages.server.minecraft.players.title', {})}
      hideTitleComponent
    >
      <ResourceView resource={details}>
        {(data) => {
          const current = page(data);
          const player = data.data;
          const version = data.gameVersion?.toLowerCase() || 'latest';

          return (
            <div className='flex flex-col gap-6'>
              {/* the page header (".theme-header"), with the head before the name */}
              <div className='theme-header'>
                <div className='theme-player-identity'>
                  <PlayerHead uuid={data.uuid} name={data.name} size='lg' />
                  <div className='min-w-0'>
                    <div className='theme-player-name'>
                      <h1 className='theme-header-heading'>{current.name}</h1>
                      {current.online === true && <Badge color='green'>{t(`${playerBase}.online`, {})}</Badge>}
                      {current.online === false && <Badge color='gray'>{t(`${playerBase}.offline`, {})}</Badge>}
                      {data.bedrock && <Badge color='gray'>{t(`${playerBase}.bedrock`, {})}</Badge>}
                    </div>
                    <div className='theme-player-meta'>
                      <Tooltip label={t(`${playerBase}.copyUuid`, {})}>
                        <CopyOnClick content={data.uuid} className='text-start'>
                          <span className='theme-player-uuid' dir='ltr'>
                            {data.uuid}
                          </span>
                        </CopyOnClick>
                      </Tooltip>
                      {data.saved && (
                        <span>
                          {tReact(`${playerBase}.saved`, { date: <FormattedTimestamp timestamp={data.saved} /> })}
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                <div className='theme-header-actions'>
                  {player?.gameMode && (
                    <Tooltip label={current.changeBlocked ?? t(`${playerBase}.gameMode`, {})}>
                      <Select
                        aria-label={t(`${playerBase}.gameMode`, {})}
                        className='w-40'
                        value={player.gameMode}
                        disabled={!!current.changeBlocked || changingMode}
                        data={GAME_MODES.map((mode) => ({ value: mode, label: gameModeName(mode) }))}
                        onChange={(value) => {
                          if (value && value !== player.gameMode) changeGameMode(current, value as GameMode);
                        }}
                      />
                    </Tooltip>
                  )}
                  <Button variant='default' onClick={back}>
                    {t(`${playerBase}.back`, {})}
                  </Button>
                  <Tooltip label={t('pages.server.minecraft.players.button.refresh', {})}>
                    <ActionIcon size='input-sm' variant='subtle' color='gray' loading={reloading} onClick={reload}>
                      <FontAwesomeIcon icon={faArrowsRotate} />
                    </ActionIcon>
                  </Tooltip>
                </div>
              </div>

              {data.damaged ? (
                <Alert color='red'>{t(`${playerBase}.damaged`, {})}</Alert>
              ) : !player ? (
                <p className='theme-player-note'>{t(`${playerBase}.noData`, {})}</p>
              ) : (
                running && <p className='theme-player-note'>{t(`${playerBase}.savedNote`, {})}</p>
              )}

              <div className='theme-player-grid'>
                <div className='theme-player-column'>
                  {player && <PlayerVitals page={current} data={player} />}
                  {player && <PlayerInventory data={player} version={version} />}
                  {!player && <PlayerAccess page={current} />}
                </div>
                <div className='theme-player-column'>
                  {player && <PlayerAccess page={current} />}
                  {player && <PlayerPosition page={current} data={player} />}
                </div>
              </div>

              <PlayerStats stats={data.stats} />

              {(canEdit || canDelete) && <DeletePlayerData page={current} />}
            </div>
          );
        }}
      </ResourceView>
    </ServerContentContainer>
  );
}
