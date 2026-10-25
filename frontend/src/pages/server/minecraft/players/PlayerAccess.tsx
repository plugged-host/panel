// plugged.host: the player on the whitelist, the operators and the bans, as switches; taking something
// away asks first, with the Players table's own dialogs (docs/features/minecraft.md#player-details)
import { useId, useState } from 'react';
import { z } from 'zod';
import Badge from '@/elements/data-display/Badge.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { minecraftPlayerListSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { PlayerPage, playerBase } from './playerPage.ts';
import { usePlayerChange } from './usePlayerChange.ts';

type List = z.infer<typeof minecraftPlayerListSchema>;

const base = 'pages.server.minecraft.players';

export default function PlayerAccess({ page }: { page: PlayerPage }) {
  const { t } = useTranslations();
  const change = usePlayerChange();
  const canManage = useServerCan('minecraft.players');
  const id = useId();
  const [openModal, setOpenModal] = useState<'unwhitelist' | 'op' | 'deop' | 'ban' | null>(null);
  const [reason, setReason] = useState('');
  const [busy, setBusy] = useState(false);

  const { details } = page;
  const name = details.name;
  const locked = !canManage || !name || busy;

  const apply = (list: List, add: boolean, banReason?: string) => {
    if (!name) return Promise.resolve(false);
    setBusy(true);
    return change(list, add, name, banReason).finally(() => setBusy(false));
  };

  const confirmed = (list: List, add: boolean, banReason?: string) =>
    apply(list, add, banReason).then((done) => {
      if (done) setOpenModal(null);
    });

  const rows: { key: string; label: string; checked: boolean; badge?: string; toggle: (on: boolean) => void }[] = [
    {
      key: 'whitelisted',
      label: t(`${playerBase}.access.whitelisted`, {}),
      checked: details.whitelisted,
      toggle: (on) => (on ? apply('whitelist', true) : setOpenModal('unwhitelist')),
    },
    {
      key: 'operator',
      label: t(`${playerBase}.access.operator`, {}),
      checked: details.operator !== null,
      badge: details.operator !== null ? t(`${base}.badge.operator`, { level: details.operator }) : undefined,
      toggle: (on) => setOpenModal(on ? 'op' : 'deop'),
    },
    {
      key: 'banned',
      label: t(`${playerBase}.access.banned`, {}),
      checked: !!details.ban,
      toggle: (on) => {
        if (on) {
          setReason('');
          setOpenModal('ban');
        } else {
          apply('bans', false);
        }
      },
    },
  ];

  const shown = name ?? page.name;

  return (
    <>
      <ConfirmationModal
        opened={openModal === 'unwhitelist'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.unwhitelist.title`, { name: shown })}
        confirm={t(`${base}.button.unwhitelist`, {})}
        onConfirmed={() => confirmed('whitelist', false)}
      >
        <Text size='sm'>{t(`${base}.modal.unwhitelist.content`, {})}</Text>
      </ConfirmationModal>

      <ConfirmationModal
        opened={openModal === 'op'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.op.title`, { name: shown })}
        confirm={t(`${base}.button.op`, {})}
        confirmColor='blue'
        onConfirmed={() => confirmed('operators', true)}
      >
        <Text size='sm'>{t(`${base}.modal.op.content`, {})}</Text>
      </ConfirmationModal>

      <ConfirmationModal
        opened={openModal === 'deop'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.deop.title`, { name: shown })}
        confirm={t(`${base}.button.deop`, {})}
        onConfirmed={() => confirmed('operators', false)}
      >
        <Text size='sm'>{t(`${base}.modal.deop.content`, {})}</Text>
      </ConfirmationModal>

      <ConfirmationModal
        opened={openModal === 'ban'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.ban.title`, { name: shown })}
        confirm={t(`${base}.button.ban`, {})}
        onConfirmed={() => confirmed('bans', true, reason.trim())}
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

      <TitleCard title={t(`${playerBase}.access.title`, {})}>
        <div className='theme-setting-rows'>
          {rows.map((row) => (
            <div key={row.key} className='theme-setting-row'>
              <label htmlFor={`${id}-${row.key}`} className='theme-player-value flex-1'>
                {row.label}
              </label>
              {row.badge && <Badge color='gray'>{row.badge}</Badge>}
              <Switch
                id={`${id}-${row.key}`}
                checked={row.checked}
                disabled={locked}
                onChange={(e) => row.toggle(e.currentTarget.checked)}
              />
            </div>
          ))}
        </div>
        {!name && <p className='theme-player-note mt-3'>{t(`${playerBase}.access.unnamed`, {})}</p>}
      </TitleCard>
    </>
  );
}
