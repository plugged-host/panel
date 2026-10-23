// plugged.host: one installed plugin or mod and what can be done with it (docs/features/minecraft.md)
import {
  faArrowUpRightFromSquare,
  faCircleArrowUp,
  faToggleOff,
  faToggleOn,
  faTrash,
} from '@fortawesome/free-solid-svg-icons';
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import deleteInstalledContent from '@/api/server/minecraft/deleteInstalledContent.ts';
import toggleInstalledContent from '@/api/server/minecraft/toggleInstalledContent.ts';
import updateInstalledContent from '@/api/server/minecraft/updateInstalledContent.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import Text from '@/elements/typography/Text.tsx';
import { bytesToString } from '@/lib/format/size.ts';
import { minecraftInstalledJarSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { minecraftSourceLabels } from './labels.ts';

interface Props {
  jar: z.infer<typeof minecraftInstalledJarSchema>;
  directory: string;
  onChanged: () => void;
}

export default function InstalledRow({ jar, directory, onChanged }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.content');

  const [openModal, setOpenModal] = useState<'delete' | 'update' | null>(null);
  const [withData, setWithData] = useState(false);
  const base = 'pages.server.minecraft.content.installed';

  const fail = (error: unknown) => {
    addToast(httpErrorToHuman(error), 'error');
    onChanged();
  };

  const toggle = () =>
    toggleInstalledContent(server.uuid, jar.file, jar.disabled)
      .then(() =>
        addToast(
          jar.disabled
            ? t(`${base}.toast.enabled`, { name: jar.name })
            : t(`${base}.toast.disabled`, { name: jar.name }),
          'success',
        ),
      )
      .then(onChanged)
      .catch(fail);

  const remove = () =>
    deleteInstalledContent(server.uuid, jar.file, withData)
      .then(() => {
        addToast(t(`${base}.toast.deleted`, { name: jar.name }), 'success');
        setOpenModal(null);
      })
      .then(onChanged)
      .catch(fail);

  const update = () =>
    updateInstalledContent(server.uuid, jar.file)
      .then(() => {
        addToast(t(`${base}.toast.updating`, { name: jar.name }), 'success');
        setOpenModal(null);
      })
      .then(onChanged)
      .catch(fail);

  return (
    <>
      <ConfirmationModal
        opened={openModal === 'delete'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.delete.title`, { name: jar.name })}
        confirm={t('common.button.delete', {})}
        onConfirmed={remove}
      >
        <Stack gap='sm'>
          <Text size='sm' component='div'>
            {t(`${base}.modal.delete.content`, { file: jar.file, directory }).md()}
          </Text>
          {jar.dataFolder && (
            <Checkbox
              label={t(`${base}.modal.delete.withData`, { folder: `${directory}/${jar.dataFolder}` })}
              description={t(`${base}.modal.delete.withDataDescription`, {})}
              checked={withData}
              onChange={(e) => setWithData(e.currentTarget.checked)}
            />
          )}
        </Stack>
      </ConfirmationModal>

      {jar.update && (
        <ConfirmationModal
          opened={openModal === 'update'}
          onClose={() => setOpenModal(null)}
          title={t(`${base}.modal.update.title`, { name: jar.name, version: jar.update.number })}
          confirm={t(`${base}.button.update`, {})}
          confirmColor='blue'
          onConfirmed={update}
        >
          <Text size='sm'>{t(`${base}.modal.update.content`, {})}</Text>
        </ConfirmationModal>
      )}

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faCircleArrowUp,
            label: t(`${base}.button.updateTo`, { version: jar.update?.number ?? '' }),
            hidden: !jar.update,
            onClick: () => setOpenModal('update'),
            color: 'gray',
            canAccess: canManage,
          },
          {
            type: 'action',
            icon: jar.disabled ? faToggleOn : faToggleOff,
            label: jar.disabled ? t(`${base}.button.enable`, {}) : t(`${base}.button.disable`, {}),
            onClick: toggle,
            color: 'gray',
            canAccess: canManage,
          },
          {
            type: 'action',
            icon: faArrowUpRightFromSquare,
            label: t('pages.server.minecraft.content.button.open', {
              source: jar.source ? minecraftSourceLabels[jar.source] : '',
            }),
            hidden: !jar.url || !jar.source,
            onClick: () => window.open(jar.url!, '_blank', 'noopener,noreferrer'),
            color: 'gray',
          },
          {
            type: 'action',
            icon: faTrash,
            label: t('common.button.delete', {}),
            onClick: () => {
              setWithData(false);
              setOpenModal('delete');
            },
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
              <Group gap='sm' wrap='nowrap'>
                {jar.icon && <img src={jar.icon} alt='' loading='lazy' className='w-8 h-8 rounded-md shrink-0' />}
                <Stack gap={0}>
                  <Group gap='xs'>
                    <Text size='sm' fw={500} c={jar.disabled ? 'dimmed' : undefined}>
                      {jar.name}
                    </Text>
                    {jar.disabled && (
                      <Badge color='gray' size='xs'>
                        {t(`${base}.badge.disabled`, {})}
                      </Badge>
                    )}
                    {jar.update && (
                      <Badge color='blue' size='xs'>
                        {t(`${base}.badge.update`, { version: jar.update.number })}
                      </Badge>
                    )}
                    {jar.missing.map((dependency) => (
                      <Tooltip key={dependency} label={t(`${base}.missingHint`, { name: dependency })}>
                        <Badge color='red' size='xs'>
                          {t(`${base}.badge.missing`, { name: dependency })}
                        </Badge>
                      </Tooltip>
                    ))}
                  </Group>
                  <Text size='xs' c='dimmed'>
                    {jar.file}
                  </Text>
                </Stack>
              </Group>
            </TableData>

            <TableData>
              {jar.pending ? (
                <Spinner size={16} />
              ) : (
                <Tooltip label={t(`${base}.confidence.${jar.confidence}`, {})}>
                  <Text size='sm' span>
                    {jar.source ? minecraftSourceLabels[jar.source] : t(`${base}.unknown`, {})}
                  </Text>
                </Tooltip>
              )}
            </TableData>

            <TableData>{jar.version ?? ''}</TableData>

            <TableData>{bytesToString(jar.size)}</TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
