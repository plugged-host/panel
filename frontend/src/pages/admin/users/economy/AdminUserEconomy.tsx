// plugged.host: a user's credits, tokens and memberships (docs/features/economy.md)
import { useQueryClient } from '@tanstack/react-query';
import { ReactNode, useState } from 'react';
import { z } from 'zod';
import getUserEconomy from '@/api/admin/users/economy/getUserEconomy.ts';
import updateUserEconomy, { UserEconomyChange } from '@/api/admin/users/economy/updateUserEconomy.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import TransactionsTable from '@/elements/economy/TransactionsTable.tsx';
import DateTimePicker from '@/elements/input/DateTimePicker.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminFullUserSchema } from '@/lib/schemas/admin/users.ts';
import { adminUserEconomySchema } from '@/lib/schemas/user/economy.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Economy = z.infer<typeof adminUserEconomySchema>;

const page = 'pages.admin.users.tabs.economy.page';

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

function ChangeForm({ user, economy }: { user: z.infer<typeof adminFullUserSchema>; economy: Economy }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();

  const [credits, setCredits] = useState<number | string>('');
  const [tokens, setTokens] = useState<number | string>('');
  const [reason, setReason] = useState('');
  const [plusplusUntil, setPlusplusUntil] = useState<Date | null>(economy.plusplusUntil);
  const [loading, setLoading] = useState(false);

  const send = (change: UserEconomyChange) => {
    setLoading(true);
    updateUserEconomy(user.uuid, change)
      .then((updated) => {
        queryClient.setQueryData(queryKeys.admin.users.economy(user.uuid), updated);
        setCredits('');
        setTokens('');
        setReason('');
        setPlusplusUntil(updated.plusplusUntil);
        addToast(t(`${page}.toast.updated`, {}), 'success');
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  const amount = (value: number | string) => (typeof value === 'number' && value !== 0 ? value : undefined);

  return (
    <Stack gap='xl'>
      <Section title={t(`${page}.balances`, {})}>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            send({
              credits: economy.creditsChangeable ? amount(credits) : undefined,
              tokens: amount(tokens),
              reason: reason.trim() || undefined,
            });
          }}
        >
          <Stack gap='sm'>
            <Group grow align='flex-start'>
              {economy.creditsChangeable && (
                <NumberInput
                  label={t(`${page}.addCredits`, {})}
                  description={t(`${page}.addDescription`, {})}
                  value={credits}
                  onChange={setCredits}
                  allowDecimal={false}
                />
              )}
              <NumberInput
                label={t(`${page}.addTokens`, {})}
                description={t(`${page}.addDescription`, {})}
                value={tokens}
                onChange={setTokens}
                allowDecimal={false}
              />
            </Group>
            <TextInput
              label={t(`${page}.reason`, {})}
              description={t(`${page}.reasonDescription`, {})}
              value={reason}
              maxLength={200}
              onChange={(e) => setReason(e.currentTarget.value)}
            />
            <Group>
              <Button type='submit' loading={loading} disabled={!amount(credits) && !amount(tokens)}>
                {t('common.button.save', {})}
              </Button>
            </Group>
          </Stack>
        </form>
      </Section>

      <Section title={t(`${page}.memberships`, {})}>
        <Group>
          <Button variant='default' loading={loading} onClick={() => send({ plus: !economy.plusSince })}>
            {economy.plusSince
              ? t(`${page}.takePlus`, { name: economy.plusName })
              : t(`${page}.givePlus`, { name: economy.plusName })}
          </Button>
        </Group>
        <Group align='flex-end'>
          <DateTimePicker
            label={t(`${page}.plusplusUntil`, { name: economy.plusplusName })}
            value={plusplusUntil}
            onChange={(value) => setPlusplusUntil(value ? new Date(value) : null)}
            clearable
          />
          <Button variant='default' loading={loading} disabled={!plusplusUntil} onClick={() => send({ plusplusUntil })}>
            {t(`${page}.setPlusplus`, {})}
          </Button>
          {economy.plusplusUntil && (
            <Button variant='default' loading={loading} onClick={() => send({ plusplusUntil: null })}>
              {t(`${page}.endPlusplus`, { name: economy.plusplusName })}
            </Button>
          )}
        </Group>
      </Section>
    </Stack>
  );
}

export default function AdminUserEconomy({ user }: { user: z.infer<typeof adminFullUserSchema> }) {
  const { t } = useTranslations();

  const economy = useResource({
    queryKey: queryKeys.admin.users.economy(user.uuid),
    queryFn: () => getUserEconomy(user.uuid),
  });

  return (
    <AdminSubContentContainer title={t(`${page}.title`, {})} titleOrder={2}>
      <ResourceView resource={economy}>
        {(economy) => (
          <Stack gap='xl'>
            <Stack gap={4}>
              <Text>
                {t(`${page}.summary`, {
                  credits: economy.credits.toLocaleString(),
                  tokens: economy.tokens.toLocaleString(),
                  earned: economy.tokensEarned.toLocaleString(),
                })}
              </Text>
              {economy.source === 'noprofit' && (
                <Text size='sm' c='dimmed'>
                  {economy.creditsChangeable ? t(`${page}.noprofitSent`, {}) : t(`${page}.noprofit`, {})}
                </Text>
              )}
              <Text size='sm' c='dimmed'>
                {economy.plusSince ? (
                  <>
                    {t(`${page}.plusSince`, { name: economy.plusName })}{' '}
                    <FormattedTimestamp timestamp={economy.plusSince} />
                  </>
                ) : (
                  t(`${page}.noPlus`, { name: economy.plusName })
                )}
              </Text>
              <Text size='sm' c='dimmed'>
                {economy.plusplusUntil ? (
                  <>
                    {t(`${page}.plusplusRuns`, { name: economy.plusplusName })}{' '}
                    <FormattedTimestamp timestamp={economy.plusplusUntil} />
                  </>
                ) : (
                  t(`${page}.noPlusplus`, { name: economy.plusplusName, purchases: economy.plusplusPurchases })
                )}
              </Text>
              {(economy.extras.cpu > 0 || economy.extras.memory > 0 || economy.extras.disk > 0) && (
                <Text size='sm' c='dimmed'>
                  {t(`${page}.extras`, {
                    cpu: economy.extras.cpu,
                    memory: bytesToString(mbToBytes(economy.extras.memory)),
                    disk: bytesToString(mbToBytes(economy.extras.disk)),
                  })}
                </Text>
              )}
            </Stack>

            <AdminCan action='users.economy'>
              <ChangeForm key={economy.plusplusUntil?.toISOString() ?? 'none'} user={user} economy={economy} />
            </AdminCan>

            <Section title={t(`${page}.creditHistory`, {})}>
              <TransactionsTable transactions={economy.creditTransactions} />
            </Section>
            <Section title={t(`${page}.tokenHistory`, {})}>
              <TransactionsTable transactions={economy.tokenTransactions} />
            </Section>
          </Stack>
        )}
      </ResourceView>
    </AdminSubContentContainer>
  );
}
