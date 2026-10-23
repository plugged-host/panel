// plugged.host: a player creating a server: game, software, location, plan, name (docs/features/create-server.md)
import { useQueryClient } from '@tanstack/react-query';
import { ReactNode, useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createServer from '@/api/server/createServer.ts';
import getServerCreateOptions from '@/api/server/getServerCreateOptions.ts';
import { AdPageSlot } from '@/elements/ads/AdPlacements.tsx';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Anchor from '@/elements/typography/Anchor.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { tierName } from '@/lib/economy.ts';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverCreateAllowanceSchema, serverCreateOptionsSchema } from '@/lib/schemas/server/create.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Options = z.infer<typeof serverCreateOptionsSchema>;

const MINECRAFT_EULA = 'https://aka.ms/MinecraftEULA';

function Choice({
  selected,
  disabled,
  onSelect,
  children,
}: {
  selected: boolean;
  disabled?: boolean;
  onSelect: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type='button'
      className='theme-choice'
      data-selected={selected || undefined}
      aria-pressed={selected}
      disabled={disabled}
      onClick={onSelect}
    >
      {children}
    </button>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Stack gap='xs'>
      <Title order={3} fz='md' fw={600}>
        {title}
      </Title>
      {children}
    </Stack>
  );
}

function AllowanceLine({ allowance }: { allowance: z.infer<typeof serverCreateAllowanceSchema> }) {
  const { t } = useTranslations();

  const left = (limit: number, used: number, format: (value: number) => string) =>
    limit > 0 ? format(Math.max(0, limit - used)) : t('common.unlimited', {});

  return (
    <Text c='dimmed' size='sm'>
      {t('pages.account.createServer.allowance', {
        cpu: left(allowance.limit.cpu, allowance.used.cpu, (value) => `${value}%`),
        memory: left(allowance.limit.memory, allowance.used.memory, (value) => bytesToString(mbToBytes(value))),
        disk: left(allowance.limit.disk, allowance.used.disk, (value) => bytesToString(mbToBytes(value))),
      })}
    </Text>
  );
}

function CreateServerForm({ options }: { options: Options }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const [eggUuid, setEggUuid] = useState(options.games[0]?.eggUuid ?? null);
  const game = options.games.find((game) => game.eggUuid === eggUuid) ?? null;
  const [variant, setVariant] = useState<string | null>(null);
  const [locationUuid, setLocationUuid] = useState(
    options.locations.find((location) => location.unlocked && location.available)?.uuid ?? null,
  );
  const [planUuid, setPlanUuid] = useState(options.plans.find((plan) => plan.available)?.uuid ?? null);
  const [name, setName] = useState('');
  const [eula, setEula] = useState(false);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setVariant(game?.variants[0]?.value ?? null);
    setEula(false);
  }, [game]);

  if (!options.enabled) {
    return <Text c='dimmed'>{t('pages.account.createServer.disabled', {})}</Text>;
  }
  if (!options.allowance.hasPlan) {
    return <Text c='dimmed'>{t('pages.account.createServer.noPlan', {})}</Text>;
  }
  if (options.games.length === 0 || options.locations.length === 0 || options.plans.length === 0) {
    return <Text c='dimmed'>{t('pages.account.createServer.nothingOffered', {})}</Text>;
  }

  const tooMany = options.allowance.maxServers > 0 && options.allowance.servers >= options.allowance.maxServers;
  const ready = !!game && !!locationUuid && !!planUuid && name.trim().length >= 3 && (!game.eula || eula) && !tooMany;

  const submit = () => {
    if (!ready || !game) return;

    setLoading(true);
    createServer({ name: name.trim(), eggUuid: game.eggUuid, locationUuid, planUuid, variant, eula })
      .then((server) => {
        queryClient.invalidateQueries({ queryKey: queryKeys.user.servers.all() });
        addToast(t('pages.account.createServer.toast.created', {}), 'success');
        navigate(`/server/${server.uuidShort}`);
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
        setLoading(false);
      });
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <Stack gap='xl'>
        <AllowanceLine allowance={options.allowance} />
        {tooMany && (
          <Text c='red' size='sm'>
            {t('pages.account.createServer.tooMany', { max: options.allowance.maxServers })}
          </Text>
        )}

        <Section title={t('pages.account.createServer.game', {})}>
          <div className='theme-choice-grid'>
            {options.games.map((option) => (
              <Choice
                key={option.eggUuid}
                selected={option.eggUuid === eggUuid}
                onSelect={() => setEggUuid(option.eggUuid)}
              >
                <span className='theme-choice-title'>{option.name}</span>
                {option.description && <span className='theme-choice-text'>{option.description}</span>}
              </Choice>
            ))}
          </div>
        </Section>

        {game && game.variants.length > 1 && (
          <Section title={t('pages.account.createServer.software', {})}>
            <div className='theme-choice-grid'>
              {game.variants.map((option) => (
                <Choice
                  key={option.value}
                  selected={option.value === variant}
                  onSelect={() => setVariant(option.value)}
                >
                  <span className='theme-choice-title'>{option.label}</span>
                </Choice>
              ))}
            </div>
          </Section>
        )}

        <Section title={t('pages.account.createServer.location', {})}>
          <div className='theme-choice-grid'>
            {options.locations.map((location) => (
              <Choice
                key={location.uuid}
                selected={location.uuid === locationUuid}
                disabled={!location.unlocked || !location.available}
                onSelect={() => setLocationUuid(location.uuid)}
              >
                <span className='theme-choice-title'>
                  {location.flag && (
                    <img src={`/flags/${location.flag}.svg`} alt='' className='w-5 h-5 rounded-md shrink-0' />
                  )}
                  {location.name}
                </span>
                <span className='theme-choice-text'>
                  {!location.unlocked
                    ? t('pages.account.createServer.locationLocked', { tier: tierName(location.tier) })
                    : !location.available
                      ? t('pages.account.createServer.locationFull', {})
                      : location.latency !== null
                        ? t('pages.account.createServer.latency', { ms: location.latency })
                        : (location.description ?? '')}
                </span>
              </Choice>
            ))}
          </div>
        </Section>

        <Section title={t('pages.account.createServer.plan', {})}>
          <div className='theme-choice-grid'>
            {options.plans.map((plan) => (
              <Choice
                key={plan.uuid}
                selected={plan.uuid === planUuid}
                disabled={!plan.available}
                onSelect={() => setPlanUuid(plan.uuid)}
              >
                <span className='theme-choice-title'>{plan.name}</span>
                <span className='theme-choice-text'>
                  {t('pages.account.createServer.planResources', {
                    cpu: plan.cpu,
                    memory: bytesToString(mbToBytes(plan.memory)),
                    disk: bytesToString(mbToBytes(plan.disk)),
                  })}
                </span>
                {!plan.available && (
                  <span className='theme-choice-text'>{t('pages.account.createServer.planTooBig', {})}</span>
                )}
              </Choice>
            ))}
          </div>
        </Section>

        <Section title={t('pages.account.createServer.details', {})}>
          <TextInput
            label={t('common.form.name', {})}
            placeholder={t('pages.account.createServer.namePlaceholder', {})}
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            maxLength={255}
            className='max-w-md'
            required
          />
          {game?.eula && (
            <Checkbox
              checked={eula}
              onChange={(e) => setEula(e.currentTarget.checked)}
              label={
                <>
                  {t('pages.account.createServer.eula', {})}{' '}
                  <Anchor href={MINECRAFT_EULA} target='_blank' rel='noreferrer'>
                    {t('pages.account.createServer.eulaLink', {})}
                  </Anchor>
                </>
              }
            />
          )}
        </Section>

        <Group>
          <Button type='submit' disabled={!ready} loading={loading}>
            {t('pages.account.createServer.submit', {})}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

export default function CreateServer() {
  const { t } = useTranslations();

  const resource = useResource({
    queryKey: queryKeys.user.servers.createOptions(),
    queryFn: getServerCreateOptions,
  });

  return (
    <AccountContentContainer title={t('pages.account.createServer.title', {})}>
      <ResourceView resource={resource}>{(options) => <CreateServerForm options={options} />}</ResourceView>
      <AdPageSlot />
    </AccountContentContainer>
  );
}
