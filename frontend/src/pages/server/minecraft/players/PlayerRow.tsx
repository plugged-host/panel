// plugged.host: one player of the Players table, the Pelican panel's row: the head and the name (a link
// to the player's page) over the UUID (copied on a click), whitelisted, operator level, ban, then the
// changes as text links (docs/features/minecraft.md)
import { faCircleCheck, faCircleXmark } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { z } from 'zod';
import UnstyledButton from '@/elements/buttons/UnstyledButton.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { minecraftPlayerListSchema, minecraftPlayerSchema } from '@/lib/schemas/server/minecraft.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import PlayerHead from './PlayerHead.tsx';
import { usePlayerChange } from './usePlayerChange.ts';

interface Props {
  player: z.infer<typeof minecraftPlayerSchema>;
  canManage: boolean;
  online: boolean;
}

export default function PlayerRow({ player, canManage, online }: Props) {
  const { t, tReact } = useTranslations();
  const change = usePlayerChange();
  const server = useServerStore((state) => state.server);

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
  // someone who played but whose name the server does not know yet: the lists need a name
  const unnamed = !player.name;

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
          <div className='theme-player-row-name'>
            <PlayerHead uuid={player.uuid} name={player.name} />
            <div className='flex min-w-0 flex-col items-start'>
              <div className='flex items-center gap-2'>
                {player.uuid ? (
                  <TableLink to={`/server/${server.uuidShort}/minecraft/players/${player.uuid}`}>
                    {player.name || t(`${base}.unnamed`, {})}
                  </TableLink>
                ) : (
                  <Text size='sm'>{player.name}</Text>
                )}
                {online && <Badge color='green'>{t(`${base}.badge.online`, {})}</Badge>}
              </div>
              {player.uuid && (
                <CopyOnClick content={player.uuid} className='text-start'>
                  <Text size='sm' c='dimmed' dir='ltr'>
                    {player.uuid}
                  </Text>
                </CopyOnClick>
              )}
              {player.lastPlayed && (
                <Text size='xs' c='dimmed'>
                  {tReact(`${base}.lastPlayed`, { date: <FormattedTimestamp timestamp={player.lastPlayed} /> })}
                </Text>
              )}
            </div>
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

        {canManage && unnamed && <TableData />}
        {canManage && !unnamed && (
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
