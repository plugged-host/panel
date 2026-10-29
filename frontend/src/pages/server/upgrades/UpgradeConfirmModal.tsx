// plugged.host: the confirmation before tokens are spent on an upgrade: what it does, its price and the
// buyer's balance; a balance that does not cover it leads to the Tokens page (docs/features/server-upgrades.md)
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import buyServerUpgrade from '@/api/server/upgrades/buyServerUpgrade.ts';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import {
  KIND_KEYS,
  type UpgradeRequest,
  type Upgrades,
  upgradesBase,
  useUpgradeDescription,
  useUpgradeDuration,
  useUpgradeRefresh,
} from './useServerUpgrades.ts';

export default function UpgradeConfirmModal({
  upgrades,
  request,
  onClose,
}: {
  upgrades: Upgrades;
  request: UpgradeRequest | null;
  onClose: () => void;
}) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const duration = useUpgradeDuration();
  const describe = useUpgradeDescription();
  const refresh = useUpgradeRefresh();

  const offer = request ? upgrades.offers.find((candidate) => candidate.kind === request.kind) : undefined;
  if (!request || !offer) return null;

  const { kind, renew, color } = request;
  const title = t(`${upgradesBase}.kinds.${KIND_KEYS[kind]}.title`, {});
  const price = tItem('token', offer.tokens);
  const balance = tItem('token', upgrades.balance);
  const short = upgrades.balance < offer.tokens;
  // a pass or a boost that runs gets longer, as does an extra slot picked to renew
  const renewing =
    !!renew ||
    ((kind === 'keep_awake' || kind === 'boost') && upgrades.purchases.some((purchase) => purchase.kind === kind));

  const confirm = short
    ? t(`${upgradesBase}.confirm.earn`, {})
    : renewing
      ? t(`${upgradesBase}.add`, { duration: duration(offer.hours), price })
      : t(`${upgradesBase}.buy`, { price });

  const doBuy = async () => {
    if (short) {
      onClose();
      navigate('/account/tokens');
      return;
    }

    try {
      const fresh = await buyServerUpgrade(server.uuid, { kind, renew, tokens: offer.tokens, color });
      refresh(fresh);

      const bought = fresh.purchases.find((purchase) => (renew ? purchase.uuid === renew : purchase.kind === kind));
      addToast(
        kind === 'card_color'
          ? t(`${upgradesBase}.toast.color`, {})
          : bought?.until
            ? t(`${upgradesBase}.toast.bought`, { title, time: formatDateTime(bought.until) })
            : t(`${upgradesBase}.toast.boughtNoEnd`, { title }),
        'success',
      );
      onClose();
    } catch (error) {
      // a price that moved or a balance that changed: show what is true now, and say why
      queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).settings.upgrades() });
      throw error;
    }
  };

  return (
    <ConfirmationModal opened onClose={onClose} title={title} confirm={confirm} confirmColor='blue' onConfirmed={doBuy}>
      <Stack gap='xs'>
        <span>{describe(offer)}</span>
        <span>
          {short
            ? t(`${upgradesBase}.confirm.short`, { price, balance })
            : t(`${upgradesBase}.confirm.price`, { price, balance })}
        </span>
      </Stack>
    </ConfirmationModal>
  );
}
