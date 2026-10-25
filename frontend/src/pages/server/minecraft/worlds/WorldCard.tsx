// plugged.host: one world as a card, the Pelican panel's: the loaded one with its generation tiles,
// Options, Generate and the upload; the others with "Use this world" and Delete
// (docs/features/minecraft.md)
import { faDownload, faFolder, faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import downloadFiles from '@/api/server/files/downloadFiles.ts';
import activateWorld from '@/api/server/minecraft/activateWorld.ts';
import deleteWorld from '@/api/server/minecraft/deleteWorld.ts';
import saveWorld from '@/api/server/minecraft/saveWorld.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ConditionalTooltip from '@/elements/overlays/ConditionalTooltip.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import Text from '@/elements/typography/Text.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { downloadUrl } from '@/lib/network/url.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftWorldSchema, minecraftWorldsSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import GenerateWorldModal from './GenerateWorldModal.tsx';
import WorldDropZone from './WorldDropZone.tsx';
import WorldOptionsModal from './WorldOptionsModal.tsx';

interface Props {
  world: z.infer<typeof minecraftWorldSchema>;
  worlds: z.infer<typeof minecraftWorldsSchema>;
  /** Why the loaded world cannot be replaced now (the server runs, a world is being imported). */
  locked: string | null;
  offline: boolean;
}

const capitalize = (value: string) => value.charAt(0).toUpperCase() + value.slice(1);

export default function WorldCard({ world, worlds, locked, offline }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.worlds');
  const canUpload = useServerCan(['minecraft.worlds', 'files.create'], false);
  const canDownload = useServerCan('files.read-content');
  const canBrowse = useServerCan('files.read');
  const canConsole = useServerCan('control.console');

  const [openModal, setOpenModal] = useState<'options' | 'generate' | 'activate' | 'delete' | null>(null);
  const [downloading, setDownloading] = useState(false);
  const base = 'pages.server.minecraft.worlds';

  const refresh = () => queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.worlds() });

  const download = async () => {
    setDownloading(true);

    try {
      // A running server keeps the last minutes of play in memory until it saves.
      if (!offline && canConsole) {
        addToast(t(`${base}.toast.saving`, {}), 'info');
        await saveWorld(server.uuid);
      }

      const { url } = await downloadFiles(server.uuid, '/', world.folders, world.folders.length === 1, 'zip');
      addToast(t('pages.server.files.toast.downloadStarted', {}), 'success');
      downloadUrl(url);
    } catch (error) {
      addToast(httpErrorToHuman(error), 'error');
    } finally {
      setDownloading(false);
    }
  };

  const activate = () =>
    activateWorld(server.uuid, world.name)
      .then(() => {
        addToast(t(`${base}.toast.activated`, { world: world.name }), 'success');
        setOpenModal(null);
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(refresh);

  const remove = () =>
    deleteWorld(server.uuid, world.name)
      .then(() => {
        addToast(t(`${base}.toast.deleted`, { world: world.name }), 'success');
        setOpenModal(null);
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(refresh);

  const { generation, gameplay } = worlds;
  const tiles = [
    { label: t(`${base}.form.seed`, {}), value: generation.seed || t(`${base}.form.seedRandom`, {}), mono: true },
    { label: t(`${base}.form.type`, {}), value: generation.levelType.slice(generation.levelType.indexOf(':') + 1) },
    { label: t(`${base}.stats.difficulty`, {}), value: capitalize(gameplay.difficulty) },
    {
      label: t(`${base}.stats.gamemode`, {}),
      value: gameplay.hardcore
        ? `${capitalize(gameplay.gamemode)} · ${t(`${base}.stats.hardcore`, {})}`
        : capitalize(gameplay.gamemode),
    },
    {
      label: t(`${base}.form.structures`, {}),
      value: generation.structures ? t(`${base}.stats.on`, {}) : t(`${base}.stats.off`, {}),
    },
    {
      label: t(`${base}.stats.modified`, {}),
      value: world.modified ? formatDateTime(world.modified) : t(`${base}.stats.never`, {}),
    },
  ];

  return (
    <section
      className='theme-world-card'
      data-other={world.active ? undefined : ''}
      data-upload={world.active && canUpload ? '' : undefined}
    >
      {world.active && (
        <>
          <WorldOptionsModal opened={openModal === 'options'} onClose={() => setOpenModal(null)} worlds={worlds} />
          <GenerateWorldModal opened={openModal === 'generate'} onClose={() => setOpenModal(null)} worlds={worlds} />
        </>
      )}

      <ConfirmationModal
        opened={openModal === 'activate'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.activate.title`, { world: world.name })}
        confirm={t(`${base}.button.activate`, {})}
        confirmColor='blue'
        onConfirmed={activate}
      >
        <Text size='sm'>{t(`${base}.modal.activate.content`, {})}</Text>
      </ConfirmationModal>

      <ConfirmationModal
        opened={openModal === 'delete'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.delete.title`, { world: world.name })}
        confirm={t(`${base}.button.delete`, {})}
        onConfirmed={remove}
      >
        <Text size='sm'>
          {worlds.trash
            ? t(`${base}.modal.delete.trash`, { folders: world.folders.join(', ') })
            : t(`${base}.modal.delete.content`, { folders: world.folders.join(', ') })}
        </Text>
      </ConfirmationModal>

      <div className='theme-world-head'>
        <div className='theme-world-title'>
          <span className='theme-world-name'>{world.name}</span>
          {world.active && <Badge color='blue'>{t(`${base}.badge.active`, {})}</Badge>}
          {!world.exists && (
            <Tooltip label={t(`${base}.badge.notGeneratedHint`, {})}>
              <Badge color='yellow'>{t(`${base}.badge.notGenerated`, {})}</Badge>
            </Tooltip>
          )}
          {world.folders.includes(`${world.name}_nether`) && (
            <Badge color='gray'>{t(`${base}.badge.nether`, {})}</Badge>
          )}
          {world.folders.includes(`${world.name}_the_end`) && (
            <Badge color='gray'>{t(`${base}.badge.theEnd`, {})}</Badge>
          )}
        </div>
        {!world.active && world.modified && (
          <p className='theme-eco-muted'>{t(`${base}.modifiedAt`, { date: formatDateTime(world.modified) })}</p>
        )}
      </div>

      {world.active && (
        <div className='theme-world-stats'>
          {tiles.map((tile) => (
            <div key={tile.label} className='theme-world-stat'>
              <span className='theme-world-stat-label'>{tile.label}</span>
              <span className='theme-world-stat-value' data-mono={tile.mono || undefined} title={tile.value}>
                {tile.value}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* The labelled actions first, then the icon-only quick actions. */}
      <div className='theme-world-actions'>
        {world.active && canManage && (
          <>
            <ConditionalTooltip enabled={!!locked} label={locked}>
              <Button variant='default' size='sm' disabled={!!locked} onClick={() => setOpenModal('options')}>
                {t(`${base}.button.options`, {})}
              </Button>
            </ConditionalTooltip>
            <ConditionalTooltip enabled={!!locked} label={locked}>
              <Button variant='default' size='sm' disabled={!!locked} onClick={() => setOpenModal('generate')}>
                {t(`${base}.button.generate`, {})}
              </Button>
            </ConditionalTooltip>
          </>
        )}
        {!world.active && canManage && (
          <Button variant='default' size='sm' onClick={() => setOpenModal('activate')}>
            {t(`${base}.button.activate`, {})}
          </Button>
        )}
        {canDownload && world.folders.length > 0 && (
          <Tooltip label={t(`${base}.button.download`, {})}>
            <ActionIcon size='input-sm' variant='subtle' color='gray' loading={downloading} onClick={download}>
              <FontAwesomeIcon icon={faDownload} />
            </ActionIcon>
          </Tooltip>
        )}
        {canBrowse && world.exists && (
          <Tooltip label={t(`${base}.button.files`, {})}>
            <ActionIcon
              size='input-sm'
              variant='subtle'
              color='gray'
              onClick={() =>
                navigate(`/server/${server.uuidShort}/files?directory=${encodeURIComponent(`/${world.name}`)}`)
              }
            >
              <FontAwesomeIcon icon={faFolder} />
            </ActionIcon>
          </Tooltip>
        )}
        {!world.active && canManage && (
          <Tooltip label={t(`${base}.button.delete`, {})}>
            <ActionIcon size='input-sm' variant='subtle' color='red' onClick={() => setOpenModal('delete')}>
              <FontAwesomeIcon icon={faTrash} />
            </ActionIcon>
          </Tooltip>
        )}
      </div>

      {world.active && canUpload && <WorldDropZone locked={locked} />}
    </section>
  );
}
