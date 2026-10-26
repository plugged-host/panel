// plugged.host: a player creating a server, laid out like the Pelican panel's create page: Details,
// Game, Location, Resources, the EULA for Minecraft, then the deploy row (docs/features/create-server.md)
import { faCircleInfo, faDiceFive, faGamepad } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Loader } from '@mantine/core';
import { useDebouncedValue, useHotkeys } from '@mantine/hooks';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { ReactNode, useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createServer from '@/api/server/createServer.ts';
import getServerCreateOptions from '@/api/server/getServerCreateOptions.ts';
import { AdPageSlot } from '@/elements/ads/AdPlacements.tsx';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Choice from '@/elements/input/Choice.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { closestLocation, useLocationPings } from '@/lib/locationPing.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { randomWord } from '@/lib/randomWord.ts';
import { serverCreateLocationSchema, serverCreateOptionsSchema } from '@/lib/schemas/server/create.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type Options = z.infer<typeof serverCreateOptionsSchema>;
type Location = z.infer<typeof serverCreateLocationSchema>;
type Plan = Options['plans'][number];
type Amount = number | '';
type Resource = 'cpu' | 'memory' | 'disk';

const page = 'pages.account.createServer';

/** The longest name, the Pelican panel's (NAME_MAX in shared/src/server_creation.rs). */
const NAME_MAX = 191;

/** A location's latency as the Pelican panel's signal meter: four bars under 40 ms, one from 150. */
function signal(ms: number): { bars: number; level: 'great' | 'good' | 'ok' | 'poor' } {
  if (ms < 40) return { bars: 4, level: 'great' };
  if (ms < 80) return { bars: 3, level: 'good' };
  if (ms < 150) return { bars: 2, level: 'ok' };
  return { bars: 1, level: 'poor' };
}

function Signal({ ms }: { ms: number }) {
  const { bars, level } = signal(ms);

  return (
    <span className='theme-choice-signal' data-level={level} title={`${ms} ms`} aria-label={`${ms} ms`}>
      <span className='theme-choice-signal-ms'>
        {ms}
        <span> ms</span>
      </span>
      {[1, 2, 3, 4].map((bar) => (
        <span key={bar} className='theme-choice-signal-bar' data-on={bar <= bars || undefined} aria-hidden />
      ))}
    </span>
  );
}

/** A location's tier: its pill with the uptime behind the (i), then what opens it or that every plan has it. */
function TierRow({ location }: { location: Location }) {
  const { t } = useTranslations();
  const economy = useGlobalStore((state) => state.settings.economy);
  const tier = location.tier || 'free';

  const pill =
    tier === 'free'
      ? t(`${page}.tiers.free`, {})
      : tier === 'plus'
        ? economy?.plusName || tier
        : tier === 'premium'
          ? t(`${page}.tiers.premium`, {})
          : tier;
  const uptime = tier === 'free' || tier === 'plus' || tier === 'premium' ? t(`${page}.uptime.${tier}`, {}) : null;
  // Plus opens the plus locations, PlusPlus the premium ones (shared/src/economy.rs)
  const membership = tier === 'plus' ? economy?.plusName : tier === 'premium' ? economy?.plusplusName : null;

  return (
    <span className='theme-choice-tier'>
      <span className='theme-choice-badge'>
        {pill}
        {uptime && (
          <Tooltip label={uptime}>
            <span className='theme-choice-badge-info' role='img' aria-label={uptime}>
              <FontAwesomeIcon icon={faCircleInfo} />
            </span>
          </Tooltip>
        )}
      </span>
      {!location.unlocked && membership && economy?.membershipEnabled ? (
        <Link to='/account/membership' className='theme-choice-unlock'>
          {t(`${page}.getTier`, { name: membership })}
        </Link>
      ) : tier === 'free' ? (
        <span className='theme-choice-note'>{t(`${page}.freeTier`, {})}</span>
      ) : null}
    </span>
  );
}

/** What is left of one resource, `null` where the allowance puts no limit on it. */
function left(limit: number, used: number): number | null {
  return limit > 0 ? Math.max(0, limit - used) : null;
}

function isAmount(value: Amount): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 1;
}

function CreateServerForm({ options }: { options: Options }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { allowance } = options;
  const remaining = {
    cpu: left(allowance.limit.cpu, allowance.used.cpu),
    memory: left(allowance.limit.memory, allowance.used.memory),
    disk: left(allowance.limit.disk, allowance.used.disk),
  };

  // the first plan that fits fills the resources in; without one, what is left
  const defaultPlan = options.plans.find((plan) => plan.available) ?? null;

  const [name, setName] = useState(() => randomWord());
  const [eggUuid, setEggUuid] = useState(options.games[0]?.eggUuid ?? null);
  const [locationUuid, setLocationUuid] = useState(
    options.locations.find((location) => location.unlocked && location.available)?.uuid ?? null,
  );
  const [planUuid, setPlanUuid] = useState<string | null>(defaultPlan?.uuid ?? null);
  const [amounts, setAmounts] = useState<Record<Resource, Amount>>({
    cpu: defaultPlan?.cpu ?? remaining.cpu ?? '',
    memory: defaultPlan?.memory ?? remaining.memory ?? '',
    disk: defaultPlan?.disk ?? remaining.disk ?? '',
  });
  const [variant, setVariant] = useState<string | null>(null);
  const [eula, setEula] = useState(true);
  const [loading, setLoading] = useState(false);

  const { cpu, memory, disk } = amounts;
  const game = options.games.find((option) => option.eggUuid === eggUuid) ?? null;
  // a game whose server setup asks for the software (Minecraft) starts on the first; the others ask here
  const asksSoftware = !!game && !game.setup && game.variants.length > 1;
  const chosenVariant =
    (asksSoftware && game.variants.find((option) => option.value === variant)?.value) ||
    game?.variants[0]?.value ||
    null;

  // the locations again for the chosen size, as the Pelican panel re-checks them: "Full", or no room
  const [sizeKey] = useDebouncedValue(
    isAmount(cpu) && isAmount(memory) && isAmount(disk) ? `${cpu}:${memory}:${disk}` : '',
    500,
  );
  const size = sizeKey ? sizeKey.split(':').map(Number) : null;
  const sizedOptions = useQuery({
    queryKey: queryKeys.user.servers.createOptionsFor(size?.[0] ?? 0, size?.[1] ?? 0, size?.[2] ?? 0),
    queryFn: () => getServerCreateOptions(size ? { cpu: size[0], memory: size[1], disk: size[2] } : undefined),
    enabled: !!size,
    placeholderData: keepPreviousData,
  });
  const locations = (size && sizedOptions.data?.locations) || options.locations;
  const location = locations.find((option) => option.uuid === locationUuid) ?? null;

  // the visitor's own ping per location (lib/locationPing.ts); until they pick one themselves, the
  // closest location that can take the server is the one selected
  const pings = useLocationPings(locations);
  const [locationPicked, setLocationPicked] = useState(false);
  const closest = locationPicked ? null : closestLocation(locations, pings);
  useEffect(() => {
    if (closest) setLocationUuid(closest);
  }, [closest]);

  const pickPlan = (plan: Plan) => {
    setPlanUuid(plan.uuid);
    setAmounts({ cpu: plan.cpu, memory: plan.memory, disk: plan.disk });
  };

  // editing a value keeps a plan selected only when all three are that plan's
  const setAmount = (resource: Resource, value: number | string) => {
    const next = { ...amounts, [resource]: typeof value === 'number' ? value : '' };

    setAmounts(next);
    setPlanUuid(
      options.plans.find((plan) => plan.cpu === next.cpu && plan.memory === next.memory && plan.disk === next.disk)
        ?.uuid ?? null,
    );
  };

  // why it cannot be deployed yet, in the Pelican panel's order: the allowance, the game, the
  // location, the name, the EULA
  const fits = (limit: number, used: number, value: number) => limit <= 0 || used + value <= limit;
  const tooMany = allowance.maxServers > 0 && allowance.servers >= allowance.maxServers;
  const reason = (() => {
    if (
      !allowance.hasPlan ||
      tooMany ||
      !isAmount(cpu) ||
      !isAmount(memory) ||
      !isAmount(disk) ||
      !fits(allowance.limit.cpu, allowance.used.cpu, cpu) ||
      !fits(allowance.limit.memory, allowance.used.memory, memory) ||
      !fits(allowance.limit.disk, allowance.used.disk, disk)
    ) {
      return t(`${page}.reason.limit`, {});
    }
    if (!game) return t(`${page}.reason.game`, {});
    if (!location || !location.unlocked || !location.available) return t(`${page}.reason.location`, {});
    if (!name.trim() || name.trim().length > NAME_MAX) {
      return t(`${page}.reason.name`, { name: t(`${page}.name`, {}), max: String(NAME_MAX) });
    }
    if (game.eula && !eula) return t(`${page}.eula`, {});

    return null;
  })();

  // Filament's danger notification: the title, then why
  const notifyFailed = (message: ReactNode) =>
    addToast(
      <div>
        <div className='font-semibold'>{t(`${page}.toast.failed`, {})}</div>
        <div>{message}</div>
      </div>,
      'error',
    );

  const submit = () => {
    if (loading) return;
    if (reason || !game || !location || !isAmount(cpu) || !isAmount(memory) || !isAmount(disk)) {
      notifyFailed(reason);
      return;
    }

    setLoading(true);
    createServer({
      name: name.trim(),
      eggUuid: game.eggUuid,
      locationUuid: location.uuid,
      planUuid,
      cpu,
      memory,
      disk,
      variant: chosenVariant,
      eula: game.eula && eula,
    })
      .then((server) => {
        queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.all() });
        navigate(`/server/${server.uuidShort}`);
      })
      .catch((error) => {
        // the API's reasons are lower case without a full stop; the notification shows a sentence
        const message = httpErrorToHuman(error);
        notifyFailed(`${message.charAt(0).toUpperCase()}${message.slice(1)}${/[.!?]$/.test(message) ? '' : '.'}`);
        setLoading(false);
      });
  };

  useHotkeys([['mod+S', submit]], []);

  const deploy = t(`${page}.deploy`, {});
  const amountInput = (resource: Resource, suffix: string) => (
    <NumberInput
      label={t(`${page}.${resource}`, {})}
      placeholder=''
      required
      min={1}
      max={remaining[resource] ?? undefined}
      allowDecimal={false}
      allowNegative={false}
      clampBehavior='none'
      hideControls
      rightSection={<span className='theme-input-suffix'>{suffix}</span>}
      value={amounts[resource]}
      onChange={(value) => setAmount(resource, value)}
    />
  );

  return (
    <AccountContentContainer
      title={t(`${page}.title`, {})}
      subtitle={t(`${page}.subtitle`, {})}
      contentRight={
        // secondary: the page's one primary is the deploy button under the form
        <Tooltip label={reason} disabled={!reason}>
          <Button variant='default' disabled={!!reason || loading} onClick={submit}>
            {deploy}
          </Button>
        </Tooltip>
      }
    >
      <div className='theme-create-page'>
        <TitleCard title={t(`${page}.details`, {})}>
          <TextInput
            label={t(`${page}.name`, {})}
            placeholder=''
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            maxLength={NAME_MAX}
            required
            rightSectionPointerEvents='all'
            rightSection={
              <Tooltip label={t(`${page}.randomName`, {})}>
                <ActionIcon variant='subtle' color='gray' onClick={() => setName((current) => randomWord(current))}>
                  <FontAwesomeIcon icon={faDiceFive} />
                </ActionIcon>
              </Tooltip>
            }
          />
        </TitleCard>

        <TitleCard title={t(`${page}.game`, {})}>
          <div className='theme-choice-grid theme-choice-grid-4'>
            {options.games.map((option) => (
              <Choice
                key={option.eggUuid}
                centered
                selected={option.eggUuid === eggUuid}
                onSelect={() => {
                  setEggUuid(option.eggUuid);
                  setVariant(null);
                }}
                media={
                  <span className='theme-choice-logo'>
                    {option.icon ? (
                      <img src={option.icon} alt={option.name} loading='lazy' />
                    ) : (
                      <FontAwesomeIcon icon={faGamepad} />
                    )}
                  </span>
                }
              >
                <span className='theme-choice-title'>{option.name}</span>
                {option.description && <span className='theme-choice-text'>{option.description}</span>}
              </Choice>
            ))}
          </div>
        </TitleCard>

        {asksSoftware && (
          <TitleCard title={t(`${page}.software`, {})} description={t(`${page}.softwareDescription`, {})}>
            <div className='theme-choice-grid theme-choice-grid-3'>
              {game.variants.map((option) => (
                <Choice
                  key={option.value}
                  radio
                  selected={option.value === chosenVariant}
                  onSelect={() => setVariant(option.value)}
                >
                  <span className='theme-choice-title'>{option.label}</span>
                </Choice>
              ))}
            </div>
          </TitleCard>
        )}

        <TitleCard title={t(`${page}.location`, {})}>
          {locations.length === 0 ? (
            <Text size='sm' c='dimmed'>
              {t(`${page}.noLocations`, {})}
            </Text>
          ) : (
            <div className='theme-choice-grid theme-choice-grid-3'>
              {locations.map((option) => {
                // a free location that cannot take the server is dimmed; a paid one stays a
                // normal card (the deploy row says why), and a locked one shows how to open it
                const disabled = !option.available && !option.tier;

                return (
                  <Choice
                    key={option.uuid}
                    radio
                    locked={!option.unlocked}
                    disabled={disabled}
                    selected={option.uuid === locationUuid && !disabled}
                    onSelect={() => {
                      setLocationUuid(option.uuid);
                      setLocationPicked(true);
                    }}
                    corner={pings[option.uuid] !== undefined ? <Signal ms={pings[option.uuid]} /> : undefined}
                    media={
                      option.flag ? (
                        <span className='theme-choice-flag' aria-hidden>
                          <img src={`/flags/${option.flag}.svg`} alt='' loading='lazy' />
                        </span>
                      ) : undefined
                    }
                  >
                    <span className='theme-choice-title'>{option.name}</span>
                    {!option.available && (
                      <span className='theme-choice-text'>
                        {option.full ? t(`${page}.full`, {}) : t(`${page}.noCapacity`, {})}
                      </span>
                    )}
                    <TierRow location={option} />
                  </Choice>
                );
              })}
            </div>
          )}
        </TitleCard>

        <TitleCard title={t(`${page}.resources`, {})} description={t(`${page}.resourcesDescription`, {})}>
          <div className='flex flex-col gap-6'>
            {options.plans.length > 0 && (
              <div className='theme-choice-grid theme-choice-grid-3'>
                {options.plans.map((plan) => (
                  <Choice
                    key={plan.uuid}
                    radio
                    disabled={!plan.available}
                    selected={plan.uuid === planUuid && plan.available}
                    onSelect={() => pickPlan(plan)}
                  >
                    <span className='theme-choice-title'>{plan.name}</span>
                    {(!plan.available || plan.description) && (
                      <span className='theme-choice-text'>
                        {plan.available ? plan.description : t(`${page}.planUnavailable`, {})}
                      </span>
                    )}
                    <span className='theme-choice-meta'>
                      {t(`${page}.planSize`, {
                        cpu: String(plan.cpu),
                        memory: String(plan.memory),
                        disk: String(plan.disk),
                      })}
                    </span>
                  </Choice>
                ))}
              </div>
            )}

            <div className='grid grid-cols-1 gap-6 sm:grid-cols-3'>
              {amountInput('cpu', '%')}
              {amountInput('memory', 'MB')}
              {amountInput('disk', 'MB')}
            </div>
          </div>
        </TitleCard>

        {game?.eula && (
          <TitleCard title={t(`${page}.eula`, {})}>
            <div className='flex flex-col gap-2'>
              <Checkbox
                aria-label={t(`${page}.eula`, {})}
                checked={eula}
                onChange={(e) => setEula(e.currentTarget.checked)}
              />
              <Text size='sm' c='dimmed'>
                {t(`${page}.eulaDescription`, {})}
              </Text>
            </div>
          </TitleCard>
        )}

        <div className='theme-deploy'>
          {reason && <span className='theme-deploy-reason'>{reason}</span>}
          <Button
            disabled={!!reason}
            leftSection={loading ? <Loader size={16} color='currentColor' /> : undefined}
            onClick={submit}
          >
            {loading ? t(`${page}.deploying`, {}) : deploy}
          </Button>
        </div>

        <AdPageSlot />
      </div>
    </AccountContentContainer>
  );
}

export default function CreateServer() {
  const { t } = useTranslations();

  const resource = useResource({
    queryKey: queryKeys.user.servers.createOptions(),
    queryFn: () => getServerCreateOptions(),
  });

  // the Pelican panel hides the page from an account that cannot create anything; here it says why
  const options = resource.data;
  const blocked = !options
    ? null
    : !options.enabled
      ? t(`${page}.disabled`, {})
      : !options.allowance.hasPlan
        ? t(`${page}.noPlan`, {})
        : options.games.length === 0
          ? t(`${page}.nothingOffered`, {})
          : null;

  if (options && !blocked) {
    return <CreateServerForm options={options} />;
  }

  return (
    <AccountContentContainer title={t(`${page}.title`, {})} subtitle={t(`${page}.subtitle`, {})}>
      {blocked ? <Text c='dimmed'>{blocked}</Text> : <ResourceView resource={resource}>{() => null}</ResourceView>}
    </AccountContentContainer>
  );
}
