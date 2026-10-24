// plugged.host: one player and what can be changed about them (docs/features/minecraft.md)
import { faBan, faCrown, faListCheck, faUnlock } from '@fortawesome/free-solid-svg-icons';
import { useState } from 'react';
import { z } from 'zod';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import Text from '@/elements/typography/Text.tsx';
import { minecraftPlayerSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { usePlayerChange } from './usePlayerChange.ts';

interface Props {
  player: z.infer<typeof minecraftPlayerSchema>;
}

export default function PlayerRow({ player }: Props) {
  const { t } = useTranslations();
  const canManage = useServerCan('minecraft.players');
  const change = usePlayerChange();

  const [openModal, setOpenModal] = useState<'op' | 'ban' | null>(null);
  const [reason, setReason] = useState('');
  const base = 'pages.server.minecraft.players';

  return (
    <>
      <ConfirmationModal
        opened={openModal === 'op'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.op.title`, { name: player.name })}
        confirm={t(`${base}.button.op`, {})}
        confirmColor='blue'
        onConfirmed={() => change('operators', true, player.name).then(() => setOpenModal(null))}
      >
        <Text size='sm'>{t(`${base}.modal.op.content`, {})}</Text>
      </ConfirmationModal>

      <ConfirmationModal
        opened={openModal === 'ban'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.ban.title`, { name: player.name })}
        confirm={t(`${base}.button.ban`, {})}
        onConfirmed={() =>
          change('bans', true, player.name, reason.trim()).then((done) => {
            if (done) setOpenModal(null);
          })
        }
      >
        <Stack gap='sm'>
          <Text size='sm'>{t(`${base}.modal.ban.content`, {})}</Text>
          <TextInput
            label={t(`${base}.form.reason`, {})}
            maxLength={200}
            value={reason}
            onChange={(e) => setReason(e.currentTarget.value)}
          />
        </Stack>
      </ConfirmationModal>

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faListCheck,
            label: player.whitelisted ? t(`${base}.button.unwhitelist`, {}) : t(`${base}.button.whitelist`, {}),
            onClick: () => change('whitelist', !player.whitelisted, player.name),
            color: 'gray',
            canAccess: canManage,
          },
          {
            type: 'action',
            icon: faCrown,
            label: player.operator !== null ? t(`${base}.button.deop`, {}) : t(`${base}.button.op`, {}),
            onClick: () => (player.operator !== null ? change('operators', false, player.name) : setOpenModal('op')),
            color: 'gray',
            canAccess: canManage,
          },
          {
            type: 'action',
            icon: faUnlock,
            label: t(`${base}.button.pardon`, {}),
            hidden: !player.ban,
            onClick: () => change('bans', false, player.name),
            color: 'gray',
            canAccess: canManage,
          },
          {
            type: 'action',
            icon: faBan,
            label: t(`${base}.button.ban`, {}),
            hidden: !!player.ban,
            onClick: () => {
              setReason('');
              setOpenModal('ban');
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
              <Stack gap={0}>
                <Text size='sm' fw={500}>
                  {player.name}
                </Text>
                <Text size='xs' c='dimmed'>
                  {player.uuid}
                </Text>
              </Stack>
            </TableData>

            <TableData>
              <Group gap='xs'>
                {player.whitelisted && (
                  <Badge color='green' size='xs'>
                    {t(`${base}.badge.whitelisted`, {})}
                  </Badge>
                )}
                {player.operator !== null && (
                  <Badge color='blue' size='xs'>
                    {t(`${base}.badge.operator`, { level: player.operator })}
                  </Badge>
                )}
                {player.ban && (
                  <Tooltip label={player.ban.reason || t(`${base}.badge.banned`, {})}>
                    <Badge color='red' size='xs'>
                      {t(`${base}.badge.banned`, {})}
                    </Badge>
                  </Tooltip>
                )}
              </Group>
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
