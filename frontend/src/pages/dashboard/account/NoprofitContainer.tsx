import { z } from 'zod';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { noprofitAccountSchema } from '@/lib/schemas/noprofit.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

// plugged.host: the noprofit.space account this user signs in with (backend shared/src/noprofit.rs).
export default function NoprofitContainer({
  account,
  accountsUrl,
}: {
  account: z.infer<typeof noprofitAccountSchema>;
  accountsUrl: string | null;
}) {
  const { t } = useTranslations();

  return (
    <TitleCard title={t('pages.account.account.containers.noprofit.title', {})} className='h-full order-10'>
      <Stack h='100%'>
        <div className='text-sm'>
          {t('pages.account.account.containers.noprofit.description', { email: account.email }).md()}
        </div>

        <dl className='grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm'>
          <dt className='text-(--mantine-color-dimmed)'>
            {t('pages.account.account.containers.noprofit.credits', {})}
          </dt>
          <dd className='font-mono'>{account.credits.toLocaleString()}</dd>

          {account.plan && (
            <>
              <dt className='text-(--mantine-color-dimmed)'>
                {t('pages.account.account.containers.noprofit.plan', {})}
              </dt>
              <dd>
                {t('pages.account.account.containers.noprofit.planValue', {
                  cpu: String(account.plan.cpu),
                  memory: bytesToString(mbToBytes(account.plan.memory)),
                  disk: bytesToString(mbToBytes(account.plan.disk)),
                })}
              </dd>
            </>
          )}

          <dt className='text-(--mantine-color-dimmed)'>
            {t('pages.account.account.containers.noprofit.discord', {})}
          </dt>
          <dd>
            {account.discord
              ? (account.discord.username ?? account.discord.id)
              : t('pages.account.account.containers.noprofit.discordNotLinked', {})}
          </dd>
        </dl>

        {accountsUrl && (
          <Group mt='auto'>
            <Button onClick={() => window.location.assign(`${accountsUrl}/settings`)}>
              {t('pages.account.account.containers.noprofit.button.manage', {})}
            </Button>
          </Group>
        )}
      </Stack>
    </TitleCard>
  );
}
