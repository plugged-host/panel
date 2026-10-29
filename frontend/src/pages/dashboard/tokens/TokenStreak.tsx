// plugged.host: the daily streak at the top of the Tokens page: "Claim today's tokens" is the page's
// filled button while today's tokens wait, and a dot per day of the week shows how far it got
// (docs/features/economy.md#daily-streak).
import { useQueryClient } from '@tanstack/react-query';
import classNames from 'classnames';
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import claimStreak from '@/api/me/economy/claimStreak.ts';
import Button from '@/elements/buttons/Button.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { tokenStreakSchema } from '@/lib/schemas/user/economy.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const page = 'pages.account.tokens.streak';
const WEEK = 7;

export default function TokenStreak({ streak }: { streak: z.infer<typeof tokenStreakSchema> }) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const [loading, setLoading] = useState(false);

  // the day's place in its week (day 9 is the second dot again) and the week's days claimed so far
  const place = ((Math.max(1, streak.day) - 1) % WEEK) + 1;
  const claimedDays = streak.claimable ? place - 1 : place;

  const claim = () => {
    setLoading(true);
    claimStreak()
      .then((claim) => {
        addToast(t(`${page}.claimed`, { tokens: tItem('token', claim.reward), day: claim.day }), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.user.tokens() });
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <div className='theme-earn-row'>
      <div className='flex min-w-0 flex-col gap-1'>
        <span className='theme-eco-title'>{t(`${page}.title`, {})}</span>
        <span className='theme-eco-muted'>
          {streak.claimable
            ? t(`${page}.today`, { day: streak.day, tokens: tItem('token', streak.reward) })
            : t(`${page}.tomorrow`, {
                day: streak.day,
                tokens: tItem('token', streak.reward),
                next: streak.nextReward,
              })}
        </span>
        <div className='theme-streak-dots' role='img' aria-label={t(`${page}.week`, { claimed: claimedDays })}>
          {Array.from({ length: WEEK }, (_, index) => (
            <span
              key={index}
              className={classNames(
                'theme-streak-dot',
                index < claimedDays && 'theme-streak-dot-done',
                streak.claimable && index === place - 1 && 'theme-streak-dot-today',
              )}
            />
          ))}
        </div>
        <span className='theme-earn-small'>{t(`${page}.hint`, {})}</span>
      </div>

      {streak.claimable && (
        <div className='theme-earn-action'>
          <Button fullWidth loading={loading} onClick={claim}>
            {t(`${page}.claim`, {})}
          </Button>
        </div>
      )}
    </div>
  );
}
