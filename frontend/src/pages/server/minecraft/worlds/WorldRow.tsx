// plugged.host: one world and what can be done with it (docs/features/minecraft.md)
import { faDownload, faFolderOpen, faRightLeft, faTrash } from '@fortawesome/free-solid-svg-icons';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import downloadFiles from '@/api/server/files/downloadFiles.ts';
import activateWorld from '@/api/server/minecraft/activateWorld.ts';
import deleteWorld from '@/api/server/minecraft/deleteWorld.ts';
import saveWorld from '@/api/server/minecraft/saveWorld.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { downloadUrl } from '@/lib/network/url.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftWorldSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

interface Props {
  world: z.infer<typeof minecraftWorldSchema>;
  running: boolean;
  trash: boolean;
}

export default function WorldRow({ world, running, trash }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.worlds');
  const canDownload = useServerCan('files.read-content');
  const canBrowse = useServerCan('files.read');
  const canConsole = useServerCan('control.console');

  const [openModal, setOpenModal] = useState<'activate' | 'delete' | null>(null);
  const base = 'pages.server.minecraft.worlds';

  const refresh = () => queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.worlds() });

  const download = async () => {
    try {
      // A running server keeps the last minutes of play in memory until it saves.
      if (running && canConsole) {
        addToast(t(`${base}.toast.saving`, {}), 'info');
        await saveWorld(server.uuid);
      }

      const { url } = await downloadFiles(server.uuid, '/', world.folders, world.folders.length === 1, 'zip');
      addToast(t('pages.server.files.toast.downloadStarted', {}), 'success');
      downloadUrl(url);
    } catch (error) {
      addToast(httpErrorToHuman(error), 'error');
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

  return (
    <>
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
          {t(`${base}.modal.delete.content`, { folders: world.folders.join(', ') })}{' '}
          {trash ? t(`${base}.modal.delete.trash`, {}) : t(`${base}.modal.delete.permanent`, {})}
        </Text>
      </ConfirmationModal>

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faDownload,
            label: t(`${base}.button.download`, {}),
            hidden: !world.exists,
            onClick: download,
            color: 'gray',
            canAccess: canDownload,
          },
          {
            type: 'action',
            icon: faRightLeft,
            label: t(`${base}.button.activate`, {}),
            hidden: world.active,
            onClick: () => setOpenModal('activate'),
            color: 'gray',
            canAccess: canManage,
          },
          {
            type: 'action',
            icon: faFolderOpen,
            label: t(`${base}.button.files`, {}),
            hidden: !world.exists,
            onClick: () =>
              navigate(`/server/${server.uuidShort}/files?directory=${encodeURIComponent(`/${world.name}`)}`),
            color: 'gray',
            canAccess: canBrowse,
          },
          {
            type: 'action',
            icon: faTrash,
            label: t(`${base}.button.delete`, {}),
            hidden: world.active,
            onClick: () => setOpenModal('delete'),
            color: 'red',
            canAccess: canManage,
          },
        ]}
      >
        {({ items, openMenu }) => (
          <TableRow
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(e.clientX, e.clientY);
            }}
          >
            <TableData>
              <Stack gap={0}>
                <Group gap='xs'>
                  <Text size='sm' fw={500}>
                    {world.name}
                  </Text>
                  {world.active && (
                    <Badge color='blue' size='xs'>
                      {t(`${base}.badge.active`, {})}
                    </Badge>
                  )}
                  {!world.exists && (
                    <Badge color='gray' size='xs'>
                      {t(`${base}.badge.notGenerated`, {})}
                    </Badge>
                  )}
                </Group>
                {world.folders.length > 1 && (
                  <Text size='xs' c='dimmed'>
                    {world.folders.join(', ')}
                  </Text>
                )}
              </Stack>
            </TableData>

            <TableData>
              {world.modified ? (
                <FormattedTimestamp timestamp={world.modified} />
              ) : (
                <Text size='sm' c='dimmed'>
                  -
                </Text>
              )}
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
