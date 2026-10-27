// plugged.host: the account's standing, once it has suspensions (docs/features/standing.md)
import { Button as MantineButton } from '@mantine/core';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import getAccountStanding from '@/api/me/account/getAccountStanding.ts';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { accountStandingSchema } from '@/lib/schemas/standing.ts';
import { standingLevelKey } from '@/lib/standing.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const page = 'pages.account.account.containers.standing';

export default function StandingContainer() {
  const { t } = useTranslations();
  const [standing, setStanding] = useState<z.infer<typeof accountStandingSchema> | null>(null);

  useEffect(() => {
    getAccountStanding()
      .then((response) => setStanding(response.standing))
      .catch(() => setStanding(null));
  }, []);

  // an account without suspensions has nothing to show
  if (!standing || (standing.level === 'good' && standing.suspensions === 0)) {
    return null;
  }

  const levelKey = standingLevelKey(standing.level);

  return (
    <TitleCard title={t(`${page}.title`, {})} className='h-full'>
      <Stack h='100%'>
        <div className='text-sm'>{t(`${page}.description`, {})}</div>

        <dl className='grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm'>
          <dt className='text-(--mantine-color-dimmed)'>{t(`${page}.level`, {})}</dt>
          <dd>{levelKey ? t(levelKey, {}) : standing.level}</dd>

          <dt className='text-(--mantine-color-dimmed)'>{t(`${page}.suspensions`, {})}</dt>
          <dd className='font-mono'>{standing.suspensions.toLocaleString()}</dd>

          <dt className='text-(--mantine-color-dimmed)'>{t(`${page}.create`, {})}</dt>
          <dd>
            {standing.canCreate
              ? t(`${page}.createAllowed`, {})
              : standing.createBlockedUntil
                ? t(`${page}.createBlockedUntil`, { date: formatDateTime(standing.createBlockedUntil) })
                : t(`${page}.createBlocked`, {})}
          </dd>
        </dl>

        {/* noprofit.space's standing page, when the standing is noprofit.space's */}
        {standing.url && (
          <Group mt='auto'>
            <MantineButton
              component='a'
              href={standing.url}
              target='_blank'
              rel='noopener noreferrer'
              variant='default'
              fw={600}
            >
              {t(`${page}.details`, {})}
            </MantineButton>
          </Group>
        )}
      </Stack>
    </TitleCard>
  );
}
