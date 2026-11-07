// plugged.host: the referral section of the Tokens page: the account's link with a copy button, how it
// pays in one line, and the friends who signed up with it (docs/features/referrals.md).
import { useQuery } from '@tanstack/react-query';
import getReferrals from '@/api/me/economy/getReferrals.ts';
import { CopyActionIcon } from '@/elements/SettingRows.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const page = 'pages.account.tokens.referrals';

export default function TokenReferrals() {
  const { t, tItem } = useTranslations();

  const { data: referrals } = useQuery({
    queryKey: queryKeys.user.referrals(),
    queryFn: getReferrals,
  });

  if (!referrals) {
    return null;
  }

  return (
    <section className='theme-eco-card'>
      <div className='flex flex-col gap-1'>
        <h3 className='theme-eco-heading'>{t(`${page}.title`, {})}</h3>
        <p className='theme-eco-muted'>
          {t(`${page}.howItWorks`, {
            threshold: tItem('token', referrals.threshold),
            reward: tItem('token', referrals.reward),
          })}
        </p>
      </div>
      <div className='flex flex-col gap-4'>
        <div className='flex flex-col gap-1'>
          <span className='theme-eco-title'>{t(`${page}.link`, {})}</span>
          <div className='flex min-w-0 items-center gap-2'>
            <span className='theme-earn-text min-w-0 truncate font-mono' dir='ltr'>
              {referrals.url}
            </span>
            <CopyActionIcon content={referrals.url} label={t(`${page}.copy`, {})} />
          </div>
          {referrals.monthlyCap > 0 && (
            <span className='theme-earn-small'>
              {t(`${page}.cap`, { friends: tItem('friend', referrals.monthlyCap) })}
            </span>
          )}
        </div>

        {referrals.friends.length === 0 ? (
          <p className='theme-eco-muted'>{t(`${page}.none`, {})}</p>
        ) : (
          <div className='flex flex-col gap-1'>
            <span className='theme-eco-title'>
              {t(`${page}.friends`, { friends: tItem('friend', referrals.total) })}
            </span>
            <div className='theme-earn-rows flex flex-col'>
              {referrals.friends.map((friend, index) => (
                <div key={`${friend.username}-${index}`} className='theme-earn-tx'>
                  <span className='theme-earn-text min-w-0 truncate'>{friend.username}</span>
                  {friend.paid ? (
                    <span className='theme-eco-amount theme-eco-amount-in'>
                      {t(`${page}.paid`, { reward: referrals.reward })}
                    </span>
                  ) : (
                    <span className='theme-earn-small'>
                      {t(`${page}.progress`, {
                        earned: Math.min(friend.tokensEarned, referrals.threshold),
                        threshold: referrals.threshold,
                      })}
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
