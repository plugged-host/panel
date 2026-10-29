// plugged.host: picking the colour of the server's card: bought once (the price shown before the
// confirmation), changed for free after that (docs/features/server-upgrades.md)
import { faCheck } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { httpErrorToHuman } from '@/api/axios.ts';
import buyServerUpgrade from '@/api/server/upgrades/buyServerUpgrade.ts';
import updateServerCardColor from '@/api/server/upgrades/updateServerCardColor.ts';
import Button from '@/elements/buttons/Button.tsx';
import UnstyledButton from '@/elements/buttons/UnstyledButton.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { type Upgrades, upgradesBase, useUpgradeDescription, useUpgradeRefresh } from './useServerUpgrades.ts';

const COLOR_NAMES = {
  blue: 'blue',
  cyan: 'cyan',
  teal: 'teal',
  green: 'green',
  yellow: 'yellow',
  orange: 'orange',
  red: 'red',
  pink: 'pink',
  grape: 'grape',
  violet: 'violet',
} as const;

export function colorName(t: ReturnType<typeof useTranslations>['t'], color: string): string {
  return color in COLOR_NAMES
    ? t(`${upgradesBase}.color.names.${COLOR_NAMES[color as keyof typeof COLOR_NAMES]}`, {})
    : color;
}

export default function CardColorModal({
  opened,
  upgrades,
  onClose,
}: {
  opened: boolean;
  upgrades: Upgrades;
  onClose: () => void;
}) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const describe = useUpgradeDescription();
  const refresh = useUpgradeRefresh();
  const [color, setColor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const bought = upgrades.purchases.some((purchase) => purchase.kind === 'card_color');
  const offer = upgrades.offers.find((candidate) => candidate.kind === 'card_color');
  const short = !bought && !!offer && upgrades.balance < offer.tokens;

  useEffect(() => {
    if (opened) setColor(upgrades.cardColor);
  }, [opened]);

  const save = () => {
    if (short) {
      onClose();
      navigate('/account/tokens');
      return;
    }
    if (!color) return;

    setLoading(true);
    (bought
      ? updateServerCardColor(server.uuid, color)
      : buyServerUpgrade(server.uuid, { kind: 'card_color', tokens: offer?.tokens ?? 0, color })
    )
      .then((fresh) => {
        refresh(fresh);
        addToast(t(`${upgradesBase}.toast.color`, {}), 'success');
        onClose();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  const price = offer ? tItem('token', offer.tokens) : '';

  return (
    <Modal opened={opened} onClose={onClose} title={t(`${upgradesBase}.kinds.cardColor.title`, {})}>
      <Stack gap='md'>
        <Stack gap='xs'>
          {bought ? (
            <span>{t(`${upgradesBase}.color.free`, {})}</span>
          ) : (
            offer && (
              <>
                <span>{describe(offer)}</span>
                <span>
                  {short
                    ? t(`${upgradesBase}.confirm.short`, { price, balance: tItem('token', upgrades.balance) })
                    : t(`${upgradesBase}.confirm.price`, { price, balance: tItem('token', upgrades.balance) })}
                </span>
              </>
            )
          )}
        </Stack>

        <div className='theme-card-swatches' role='group' aria-label={t(`${upgradesBase}.color.pick`, {})}>
          {upgrades.palette.map((choice) => (
            <UnstyledButton
              key={choice}
              className='theme-card-swatch'
              data-card-color={choice}
              aria-label={colorName(t, choice)}
              aria-pressed={color === choice}
              title={colorName(t, choice)}
              onClick={() => setColor(choice)}
            >
              {color === choice && <FontAwesomeIcon icon={faCheck} />}
            </UnstyledButton>
          ))}
        </div>
      </Stack>

      <ModalFooter>
        <Button
          loading={loading}
          disabled={!short && (!color || (bought && color === upgrades.cardColor))}
          onClick={save}
        >
          {short
            ? t(`${upgradesBase}.confirm.earn`, {})
            : bought
              ? t(`${upgradesBase}.color.save`, {})
              : t(`${upgradesBase}.buy`, { price })}
        </Button>
        <Button variant='default' onClick={onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
