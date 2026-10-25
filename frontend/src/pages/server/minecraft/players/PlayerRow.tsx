// plugged.host: one player of the Players table, the Pelican panel's row: the name (copied on a
// click) over the UUID, whitelisted, operator level, ban, then the changes as text links
// (docs/features/minecraft.md)
import { faCircleCheck, faCircleXmark } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { z } from 'zod';
import UnstyledButton from '@/elements/buttons/UnstyledButton.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import Text from '@/elements/typography/Text.tsx';
import { minecraftPlayerListSchema, minecraftPlayerSchema } from '@/lib/schemas/server/minecraft.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { usePlayerChange } from './usePlayerChange.ts';

interface Props {
  player: z.infer<typeof minecraftPlayerSchema>;
  canManage: boolean;
}

export default function PlayerRow({ player, canManage }: Props) {
  const { t } = useTranslations();
  const change = usePlayerChange();

  const [openModal, setOpenModal] = useState<'unwhitelist' | 'op' | 'deop' | 'ban' | null>(null);
  const [reason, setReason] = useState('');
  const [busy, setBusy] = useState(false);
  const base = 'pages.server.minecraft.players';

  // Granting goes straight through; taking something away asks first, in its modal.
  const apply = (list: z.infer<typeof minecraftPlayerListSchema>, add: boolean) => {
    setBusy(true);
    return change(list, add, player.name).finally(() => setBusy(false));
  };

  const confirmed = (list: z.infer<typeof minecraftPlayerListSchema>, add: boolean) =>
    apply(list, add).then(() => setOpenModal(null));

  const placeholder = <span className='theme-placeholder'>—</span>;

  return (
    <>
      <ConfirmationModal
        opened={openModal === 'unwhitelist'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.unwhitelist.title`, { name: player.name })}
        confirm={t(`${base}.button.unwhitelist`, {})}
        onConfirmed={() => confirmed('whitelist', false)}
      >
        <Text size='sm'>{t(`${base}.modal.unwhitelist.content`, {})}</Text>
      </ConfirmationModal>

      <ConfirmationModal
        opened={openModal === 'op'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.op.title`, { name: player.name })}
        confirm={t(`${base}.button.op`, {})}
        confirmColor='blue'
        onConfirmed={() => confirmed('operators', true)}
      >
        <Text size='sm'>{t(`${base}.modal.op.content`, {})}</Text>
      </ConfirmationModal>

      <ConfirmationModal
        opened={openModal === 'deop'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.deop.title`, { name: player.name })}
        confirm={t(`${base}.button.deop`, {})}
        onConfirmed={() => confirmed('operators', false)}
      >
        <Text size='sm'>{t(`${base}.modal.deop.content`, {})}</Text>
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

      <TableRow>
        <TableData>
          <div className='flex flex-col items-start'>
            <CopyOnClick content={player.name} className='text-start'>
              <Text size='sm'>{player.name}</Text>
            </CopyOnClick>
            {player.uuid && (
              <Text size='sm' c='dimmed'>
                {player.uuid}
              </Text>
            )}
          </div>
        </TableData>

        <TableData>
          <FontAwesomeIcon
            icon={player.whitelisted ? faCircleCheck : faCircleXmark}
            className='theme-boolean'
            data-on={player.whitelisted || undefined}
            title={player.whitelisted ? t('common.yes', {}) : t('common.no', {})}
          />
        </TableData>

        <TableData>
          {player.operator !== null ? (
            <Badge color='gray'>{t(`${base}.badge.operator`, { level: player.operator })}</Badge>
          ) : (
            placeholder
          )}
        </TableData>

        <TableData>
          {player.ban ? (
            <Tooltip label={player.ban.reason || t(`${base}.badge.banned`, {})}>
              <Badge color='red'>{t(`${base}.badge.banned`, {})}</Badge>
            </Tooltip>
          ) : (
            placeholder
          )}
        </TableData>

        {canManage && (
          <TableData className='text-end'>
            <div className='theme-link-actions'>
              {player.whitelisted ? (
                <UnstyledButton className='theme-link-action' data-danger onClick={() => setOpenModal('unwhitelist')}>
                  {t(`${base}.button.unwhitelist`, {})}
                </UnstyledButton>
              ) : (
                <UnstyledButton className='theme-link-action' disabled={busy} onClick={() => apply('whitelist', true)}>
                  {t(`${base}.button.whitelist`, {})}
                </UnstyledButton>
              )}
              {player.operator !== null ? (
                <UnstyledButton className='theme-link-action' data-danger onClick={() => setOpenModal('deop')}>
                  {t(`${base}.button.deop`, {})}
                </UnstyledButton>
              ) : (
                <UnstyledButton className='theme-link-action' onClick={() => setOpenModal('op')}>
                  {t(`${base}.button.op`, {})}
                </UnstyledButton>
              )}
              {player.ban ? (
                <UnstyledButton className='theme-link-action' disabled={busy} onClick={() => apply('bans', false)}>
                  {t(`${base}.button.pardon`, {})}
                </UnstyledButton>
              ) : (
                <UnstyledButton
                  className='theme-link-action'
                  data-danger
                  onClick={() => {
                    setReason('');
                    setOpenModal('ban');
                  }}
                >
                  {t(`${base}.button.ban`, {})}
                </UnstyledButton>
              )}
            </div>
          </TableData>
        )}
      </TableRow>
    </>
  );
}
