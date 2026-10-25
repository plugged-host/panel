// plugged.host: where a player is, their respawn point and last death, and the Teleport dialog
// (docs/features/minecraft.md#player-details)
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getOnlinePlayers from '@/api/server/minecraft/getOnlinePlayers.ts';
import getPlayers from '@/api/server/minecraft/getPlayers.ts';
import teleportPlayer, { TeleportDestination } from '@/api/server/minecraft/teleportPlayer.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Select from '@/elements/input/Select.tsx';
import Group from '@/elements/layout/Group.tsx';
import SegmentedControl from '@/elements/layout/SegmentedControl.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { formatNumber } from '@/lib/format/serverUsage.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftPositionSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { PlayerData, PlayerPage, playerBase } from './playerPage.ts';

type Position = z.infer<typeof minecraftPositionSchema>;
type Kind = TeleportDestination['kind'];

const DIMENSIONS = ['minecraft:overworld', 'minecraft:the_nether', 'minecraft:the_end'] as const;
/** The dimension the player is in. */
const CURRENT = 'current';

/** A dimension's name: the three of the game translated, a datapack's by its id. */
export function useDimensionName() {
  const { t } = useTranslations();

  return (id: string) => {
    switch (id) {
      case 'minecraft:overworld':
        return t(`${playerBase}.position.overworld`, {});
      case 'minecraft:the_nether':
        return t(`${playerBase}.position.nether`, {});
      case 'minecraft:the_end':
        return t(`${playerBase}.position.end`, {});
      default:
        return id;
    }
  };
}

const coordinates = (position: Position) =>
  [position.x, position.y, position.z].map((value) => formatNumber(value, 1)).join(', ');

function TeleportModal({
  page,
  data,
  opened,
  onClose,
}: {
  page: PlayerPage;
  data: PlayerData;
  opened: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const dimensionName = useDimensionName();

  const [kind, setKind] = useState<Kind>('coordinates');
  const [x, setX] = useState<number | string>(Math.floor(data.position?.x ?? 0));
  const [y, setY] = useState<number | string>(Math.floor(data.position?.y ?? 64));
  const [z, setZ] = useState<number | string>(Math.floor(data.position?.z ?? 0));
  const [dimension, setDimension] = useState<string>(CURRENT);
  const [target, setTarget] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // while the server runs, the players on it; while it is stopped, everyone with a saved place
  const online = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.online(),
    queryFn: () => getOnlinePlayers(server.uuid),
    enabled: opened && page.running,
    silent: true,
  });
  const roster = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.players(),
    queryFn: () => getPlayers(server.uuid),
    enabled: opened && !page.running,
    silent: true,
  });

  const self = page.details.uuid;
  const targets = page.running
    ? (online.data?.players ?? [])
        .filter((player) => player.uuid && player.uuid.toLowerCase() !== self)
        .map((player) => ({ value: player.uuid?.toLowerCase() ?? '', label: player.name }))
    : (roster.data?.players ?? [])
        .filter((player) => player.uuid && player.uuid.toLowerCase() !== self && player.lastPlayed)
        .map((player) => ({ value: player.uuid.toLowerCase(), label: player.name || player.uuid }));

  const numbers = [x, y, z].map((value) => (typeof value === 'number' ? value : Number.parseFloat(value)));
  const valid =
    kind === 'spawn' ||
    (kind === 'player' && !!target) ||
    (kind === 'coordinates' && numbers.every((value) => Number.isFinite(value)));

  const submit = () => {
    const destination: TeleportDestination =
      kind === 'coordinates'
        ? { kind, x: numbers[0], y: numbers[1], z: numbers[2], dimension: dimension === CURRENT ? null : dimension }
        : kind === 'spawn'
          ? { kind }
          : { kind, uuid: target ?? '' };

    setLoading(true);
    teleportPlayer(server.uuid, self, destination)
      .then((live) => {
        page.done(t(`${playerBase}.toast.teleported`, { name: page.name }), live);
        if (!live) {
          page.reload();
        } else if (destination.kind === 'coordinates') {
          page.patch((current) => ({
            ...current,
            position: {
              x: destination.x,
              y: destination.y,
              z: destination.z,
              dimension: destination.dimension ?? current.position?.dimension ?? DIMENSIONS[0],
            },
          }));
        }
        onClose();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal opened={opened} onClose={onClose} title={t(`${playerBase}.teleport.title`, { name: page.name })}>
      <Stack gap='md'>
        <SegmentedControl
          fullWidth
          value={kind}
          onChange={(value) => setKind(value as Kind)}
          data={[
            { value: 'coordinates', label: t(`${playerBase}.teleport.coordinates`, {}) },
            { value: 'spawn', label: t(`${playerBase}.teleport.spawn`, {}) },
            { value: 'player', label: t(`${playerBase}.teleport.player`, {}) },
          ]}
        />

        {kind === 'coordinates' && (
          <>
            <Group grow gap='sm'>
              <NumberInput label='X' value={x} onChange={setX} decimalScale={3} min={-30000000} max={30000000} />
              <NumberInput label='Y' value={y} onChange={setY} decimalScale={3} min={-30000000} max={30000000} />
              <NumberInput label='Z' value={z} onChange={setZ} decimalScale={3} min={-30000000} max={30000000} />
            </Group>
            <Select
              label={t(`${playerBase}.position.dimension`, {})}
              value={dimension}
              onChange={(value) => setDimension(value ?? CURRENT)}
              data={[
                { value: CURRENT, label: t(`${playerBase}.teleport.current`, {}) },
                ...DIMENSIONS.map((id) => ({ value: id, label: dimensionName(id) })),
              ]}
            />
          </>
        )}

        {kind === 'spawn' && <Text size='sm'>{t(`${playerBase}.teleport.spawnHint`, {})}</Text>}

        {kind === 'player' &&
          (targets.length > 0 ? (
            <Select
              label={t(`${playerBase}.teleport.target`, {})}
              value={target}
              onChange={setTarget}
              data={targets}
              searchable
            />
          ) : (
            <Text size='sm'>{t(`${playerBase}.teleport.noTargets`, {})}</Text>
          ))}

        <Text size='sm' c='dimmed'>
          {page.running ? t(`${playerBase}.teleport.live`, {}) : t(`${playerBase}.teleport.file`, {})}
        </Text>
      </Stack>

      <ModalFooter>
        <Button loading={loading} disabled={!valid} onClick={submit}>
          {t(`${playerBase}.teleport.submit`, {})}
        </Button>
        <Button variant='default' onClick={onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

export default function PlayerPosition({ page, data }: { page: PlayerPage; data: PlayerData }) {
  const { t } = useTranslations();
  const canConsole = useServerCan('control.console');
  const dimensionName = useDimensionName();
  const [teleporting, setTeleporting] = useState(false);

  const place = (position: Position | null) =>
    position ? (
      <span className='theme-player-value' data-mono dir='ltr'>
        {coordinates(position)} · {dimensionName(position.dimension)}
      </span>
    ) : (
      <span className='theme-placeholder'>{t(`${playerBase}.position.unknown`, {})}</span>
    );

  return (
    <>
      {teleporting && <TeleportModal page={page} data={data} opened onClose={() => setTeleporting(false)} />}

      <TitleCard
        title={t(`${playerBase}.position.title`, {})}
        footer={
          canConsole || page.canEdit ? (
            <>
              {page.changeBlocked && <p className='theme-player-note me-auto self-center'>{page.changeBlocked}</p>}
              <Button
                variant='default'
                size='sm'
                disabled={!!page.changeBlocked || !data.position}
                onClick={() => setTeleporting(true)}
              >
                {t(`${playerBase}.position.teleport`, {})}
              </Button>
            </>
          ) : undefined
        }
      >
        <div className='theme-setting-rows'>
          <div className='theme-setting-row'>
            <span className='theme-setting-label'>{t(`${playerBase}.position.coordinates`, {})}</span>
            {data.position ? (
              <span className='theme-player-value' data-mono dir='ltr'>
                {coordinates(data.position)}
              </span>
            ) : (
              <span className='theme-placeholder'>{t(`${playerBase}.position.unknown`, {})}</span>
            )}
          </div>
          <div className='theme-setting-row'>
            <span className='theme-setting-label'>{t(`${playerBase}.position.dimension`, {})}</span>
            {data.position ? (
              <span className='theme-player-value'>{dimensionName(data.position.dimension)}</span>
            ) : (
              <span className='theme-placeholder'>{t(`${playerBase}.position.unknown`, {})}</span>
            )}
          </div>
          <div className='theme-setting-row'>
            <span className='theme-setting-label'>{t(`${playerBase}.position.respawn`, {})}</span>
            {place(data.respawn)}
          </div>
          <div className='theme-setting-row'>
            <span className='theme-setting-label'>{t(`${playerBase}.position.lastDeath`, {})}</span>
            {place(data.lastDeath)}
          </div>
        </div>
      </TitleCard>
    </>
  );
}
