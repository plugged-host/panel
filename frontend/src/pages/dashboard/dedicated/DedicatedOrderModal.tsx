// plugged.host: ordering a paid plan on the panel, its first month paid with noprofit.space Funds (docs/features/dedicated.md)
import { faDiceFive } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ReactNode, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getBalance from '@/api/me/economy/getBalance.ts';
import getServerCreateOptions from '@/api/server/getServerCreateOptions.ts';
import orderDedicatedPlan from '@/api/server/orderDedicatedPlan.ts';
import AddFundsLink, { formatEuro } from '@/elements/AddFundsLink.tsx';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { randomWord } from '@/lib/randomWord.ts';
import { serverCreateDedicatedPlanSchema } from '@/lib/schemas/server/create.ts';
import { SETUP_PATH } from '@/pages/server/minecraft/setup/useMinecraftSetup.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type DedicatedPlan = z.infer<typeof serverCreateDedicatedPlanSchema>;

const page = 'pages.account.dedicated';
const create = 'pages.account.createServer';

/** The longest name, the create page's (NAME_MAX in shared/src/server_creation.rs). */
const NAME_MAX = 191;

export default function DedicatedOrderModal({
  plan,
  price,
  onClose,
}: {
  plan: DedicatedPlan;
  /** The plan's price, euro cents a month. */
  price: number;
  onClose: () => void;
}) {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // the create page's games, and the sidebar's balance (the same caches)
  const options = useQuery({
    queryKey: queryKeys.user.servers.createOptions(),
    queryFn: () => getServerCreateOptions(),
  });
  const balance = useQuery({
    queryKey: queryKeys.user.balance(),
    queryFn: getBalance,
    refetchInterval: 60_000,
  });

  const games = options.data?.games ?? [];
  const locations = plan.locations.filter((location) => location.available);

  const [name, setName] = useState(() => randomWord());
  const [eggUuid, setEggUuid] = useState<string | null>(null);
  const [variant, setVariant] = useState<string | null>(null);
  const [locationUuid, setLocationUuid] = useState<string | null>(locations[0]?.uuid ?? null);
  const [eula, setEula] = useState(false);
  const [loading, setLoading] = useState(false);

  const game = games.find((option) => option.eggUuid === eggUuid) ?? games[0] ?? null;
  // as on the create page: a game whose setup asks for the software (Minecraft) starts on the first
  const asksSoftware = !!game && !game.setup && game.variants.length > 1;
  const chosenVariant =
    (asksSoftware && game.variants.find((option) => option.value === variant)?.value) ||
    game?.variants[0]?.value ||
    null;
  const location = locations.find((option) => option.uuid === locationUuid) ?? null;

  // Funds are noprofit.space's (null without a linked account): nothing to pay with then
  const funds = balance.data ? (balance.data.funds ?? 0) : null;
  const short = funds !== null && funds < price;
  const priceText = formatEuro(price, language);

  const reason = (() => {
    if (!game) return t(`${create}.reason.game`, {});
    if (!location) return t(`${create}.reason.location`, {});
    if (!name.trim() || name.trim().length > NAME_MAX) {
      return t(`${create}.reason.name`, { name: t(`${create}.name`, {}), max: String(NAME_MAX) });
    }
    if (game.eula && !eula) return t(`${create}.eula`, {});
    if (short) return t(`${page}.modal.short`, {});

    return null;
  })();

  // the create page's danger notification: the title, then why
  const notifyFailed = (message: ReactNode) =>
    addToast(
      <div>
        <div className='font-semibold'>{t(`${create}.toast.failed`, {})}</div>
        <div>{message}</div>
      </div>,
      'error',
    );

  const submit = () => {
    if (loading || reason || funds === null || !game || !location) return;

    setLoading(true);
    orderDedicatedPlan({
      planUuid: plan.uuid,
      locationUuid: location.uuid,
      name: name.trim(),
      eggUuid: game.eggUuid,
      variant: chosenVariant,
      eula: game.eula && eula,
    })
      .then((server) => {
        queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.all() });
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
        addToast(t(`${page}.modal.paid`, { price: priceText }), 'success');
        onClose();
        // a server that waits for its setup opens on it (docs/features/minecraft.md#setup)
        navigate(`/server/${server.uuidShort}${game.setup ? SETUP_PATH : ''}`);
      })
      .catch((error) => {
        // the API's reasons are lower case without a full stop; the notification shows a sentence
        const message = httpErrorToHuman(error);
        notifyFailed(`${message.charAt(0).toUpperCase()}${message.slice(1)}${/[.!?]$/.test(message) ? '' : '.'}`);
        // the Funds or the plan's room may have changed meanwhile
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
        queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.dedicated() });
        setLoading(false);
      });
  };

  return (
    <Modal opened onClose={onClose} title={t(`${page}.modal.title`, { plan: plan.name })} closeOnEscape={!loading}>
      {options.isLoading ? (
        <Spinner.Centered />
      ) : (
        <Stack>
          <TextInput
            label={t(`${create}.name`, {})}
            placeholder=''
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            maxLength={NAME_MAX}
            required
            rightSectionPointerEvents='all'
            rightSection={
              <Tooltip label={t(`${create}.randomName`, {})}>
                <ActionIcon variant='subtle' color='gray' onClick={() => setName((current) => randomWord(current))}>
                  <FontAwesomeIcon icon={faDiceFive} />
                </ActionIcon>
              </Tooltip>
            }
          />

          {games.length === 0 ? (
            <Text size='sm' c='dimmed'>
              {t(`${create}.nothingOffered`, {})}
            </Text>
          ) : (
            <Select
              label={t(`${create}.game`, {})}
              required
              data={games.map((option) => ({ value: option.eggUuid, label: option.name }))}
              value={game?.eggUuid ?? null}
              onChange={(value) => {
                setEggUuid(value);
                setVariant(null);
              }}
            />
          )}

          {asksSoftware && (
            <Select
              label={t(`${create}.software`, {})}
              description={t(`${create}.softwareDescription`, {})}
              data={game.variants}
              value={chosenVariant}
              onChange={setVariant}
            />
          )}

          <Select
            label={t(`${create}.location`, {})}
            required
            data={locations.map((option) => ({ value: option.uuid, label: option.name }))}
            value={location?.uuid ?? null}
            onChange={setLocationUuid}
          />

          {game?.eula && (
            <Checkbox
              label={t(`${create}.eula`, {})}
              description={t(`${create}.eulaDescription`, {})}
              checked={eula}
              onChange={(e) => setEula(e.currentTarget.checked)}
            />
          )}

          <Stack gap={4}>
            <Text size='sm'>{t(`${page}.modal.price`, { price: priceText })}</Text>
            <Text size='sm' c='dimmed'>
              {funds === null
                ? t(`${page}.modal.fundsLoading`, {})
                : t(`${page}.modal.funds`, { funds: formatEuro(funds, language) })}
            </Text>
            <Text size='sm' c='dimmed'>
              {t(`${page}.modal.renewal`, {})}
            </Text>
          </Stack>

          {short && (
            <Stack gap={4}>
              <Text size='sm'>{t(`${page}.modal.short`, {})}</Text>
              <div>
                <AddFundsLink />
              </div>
            </Stack>
          )}
        </Stack>
      )}

      <ModalFooter>
        <Tooltip label={reason} disabled={!reason}>
          <span>
            <Button disabled={!!reason || funds === null || options.isLoading} loading={loading} onClick={submit}>
              {t(`${page}.modal.pay`, { price: priceText })}
            </Button>
          </span>
        </Tooltip>
        <Button variant='default' onClick={onClose} disabled={loading}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
