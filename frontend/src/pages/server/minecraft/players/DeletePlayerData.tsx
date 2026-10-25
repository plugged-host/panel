// plugged.host: delete parts of a player's saved data while the server is stopped: one list of what
// can go, one button, a confirmation that says what is kept (docs/features/minecraft.md#player-details)
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import deletePlayerData, { PlayerDataRemoval } from '@/api/server/minecraft/deletePlayerData.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { PlayerPage, playerBase } from './playerPage.ts';

const NOTHING: PlayerDataRemoval = {
  xp: false,
  inventory: false,
  enderChest: false,
  playerData: false,
  statistics: false,
  advancements: false,
};

/** What is taken out of the player data file rather than deleted with it. */
const EDITS = ['xp', 'inventory', 'enderChest'] as const;

export default function DeletePlayerData({ page }: { page: PlayerPage }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const [chosen, setChosen] = useState<PlayerDataRemoval>(NOTHING);
  const [confirming, setConfirming] = useState(false);

  const { details } = page;
  const files = details.files;

  const options: { key: keyof PlayerDataRemoval; label: string; hint?: string; there: boolean; allowed: boolean }[] = [
    { key: 'xp', label: t(`${playerBase}.remove.xp`, {}), there: files.playerData, allowed: page.canEdit },
    {
      key: 'inventory',
      label: t(`${playerBase}.remove.inventory`, {}),
      there: files.playerData,
      allowed: page.canEdit,
    },
    {
      key: 'enderChest',
      label: t(`${playerBase}.remove.enderChest`, {}),
      there: files.playerData,
      allowed: page.canEdit,
    },
    {
      key: 'playerData',
      label: t(`${playerBase}.remove.playerData`, {}),
      hint: t(`${playerBase}.remove.playerDataHint`, {}),
      there: files.playerData,
      allowed: page.canDelete,
    },
    {
      key: 'statistics',
      label: t(`${playerBase}.remove.statistics`, {}),
      there: files.statistics,
      allowed: page.canDelete,
    },
    {
      key: 'advancements',
      label: t(`${playerBase}.remove.advancements`, {}),
      there: files.advancements,
      allowed: page.canDelete,
    },
  ];

  // the whole file takes what is in it along
  const implied = (key: keyof PlayerDataRemoval) => chosen.playerData && (EDITS as readonly string[]).includes(key);
  const picked = options.filter((option) => chosen[option.key] || implied(option.key));
  const edits = !chosen.playerData && EDITS.some((key) => chosen[key]);
  const deletes = chosen.playerData || chosen.statistics || chosen.advancements;

  const remove = () =>
    deletePlayerData(server.uuid, details.uuid, chosen)
      .then(() => {
        addToast(t(`${playerBase}.toast.deleted`, { name: page.name }), 'success');
        setChosen(NOTHING);
        setConfirming(false);
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.players() }));

  return (
    <>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t(`${playerBase}.remove.confirmTitle`, { name: page.name })}
        confirm={t(`${playerBase}.remove.submit`, {})}
        onConfirmed={remove}
      >
        <Stack gap='sm'>
          <ul className='list-disc ps-5 text-sm'>
            {picked.map((option) => (
              <li key={option.key}>{option.label}</li>
            ))}
          </ul>
          {edits && <Text size='sm'>{t(`${playerBase}.remove.edited`, { file: `${details.uuid}.dat_old` })}</Text>}
          {deletes && (
            <Text size='sm'>
              {details.trash ? t(`${playerBase}.remove.trash`, {}) : t(`${playerBase}.remove.gone`, {})}
            </Text>
          )}
        </Stack>
      </ConfirmationModal>

      <TitleCard
        danger
        title={t(`${playerBase}.remove.title`, {})}
        description={t(`${playerBase}.remove.description`, {})}
        footer={
          <>
            {!page.stopped && (
              <p className='theme-player-note me-auto self-center'>{t(`${playerBase}.reason.stopFirst`, {})}</p>
            )}
            <Button
              variant='default'
              size='sm'
              disabled={!page.stopped || picked.length === 0}
              onClick={() => setConfirming(true)}
            >
              {t(`${playerBase}.remove.submit`, {})}
            </Button>
          </>
        }
      >
        <Stack gap='sm'>
          {options.map((option) => (
            <Checkbox
              key={option.key}
              label={option.label}
              description={option.there ? option.hint : t(`${playerBase}.remove.missing`, {})}
              checked={chosen[option.key] || implied(option.key)}
              disabled={!page.stopped || !option.there || !option.allowed || implied(option.key)}
              onChange={(e) => {
                const checked = e.currentTarget.checked;
                setChosen((current) => ({ ...current, [option.key]: checked }));
              }}
            />
          ))}
        </Stack>
      </TitleCard>
    </>
  );
}
