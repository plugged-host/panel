// plugged.host: profile badges (docs/features/promos.md#profile-badges): a few preset badges bought once
// with tokens, one of them shown next to the account's name (the sidebar, a server's subusers, its chat)
import { AxiosError } from 'axios';
import { useEffect, useState } from 'react';
import { Link } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import buyBadge from '@/api/me/economy/buyBadge.ts';
import getBadges from '@/api/me/economy/getBadges.ts';
import showBadge from '@/api/me/economy/showBadge.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import ProfileBadge from '@/elements/ProfileBadge.tsx';
import { userBadgesSchema } from '@/lib/schemas/user/badges.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const page = 'pages.account.account.containers.badges';

type Badges = z.infer<typeof userBadgesSchema>;

export default function BadgesContainer() {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const { user, setUser } = useAuth();
  const [badges, setBadges] = useState<Badges | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    getBadges()
      .then(setBadges)
      .catch(() => setBadges(null));
  }, []);

  // nothing sold and nothing owned: no card
  if (!badges || (!badges.enabled && badges.owned.length === 0)) {
    return null;
  }

  const apply = (next: Badges) => {
    setBadges(next);
    if (user) setUser({ ...user, badge: next.shown });
  };

  const doBuy = (badge: string) => {
    setBusy(badge);
    buyBadge(badge, badges.tokens)
      .then((next) => {
        apply(next);
        addToast(t(`${page}.toast.bought`, {}), 'success');
      })
      .catch((err) => {
        if (err instanceof AxiosError && err.response?.status === 409) {
          getBadges()
            .then(setBadges)
            .catch(() => undefined);
        }
        addToast(httpErrorToHuman(err), 'error');
      })
      .finally(() => setBusy(null));
  };

  const doShow = (badge: string | null) => {
    setBusy(badge ?? 'none');
    showBadge(badge)
      .then(apply)
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setBusy(null));
  };

  const short = badges.balance < badges.tokens;
  const listed = badges.enabled ? badges.available : badges.owned;

  return (
    <TitleCard title={t(`${page}.title`, {})} className='h-full'>
      <div className='flex flex-col gap-4'>
        <p className='text-sm'>
          {badges.enabled
            ? t(`${page}.description`, { tokens: tItem('token', badges.tokens) })
            : t(`${page}.descriptionOwned`, {})}
        </p>

        <ul className='theme-badge-rows'>
          {listed.map((badge) => {
            const owned = badges.owned.includes(badge);
            const shown = badges.shown === badge;

            return (
              <li key={badge} className='theme-badge-row'>
                <ProfileBadge badge={badge} />
                {shown ? (
                  <span className='theme-eco-small'>{t(`${page}.shown`, {})}</span>
                ) : owned ? (
                  <Button size='xs' variant='default' loading={busy === badge} onClick={() => doShow(badge)}>
                    {t(`${page}.show`, {})}
                  </Button>
                ) : (
                  <Button
                    size='xs'
                    variant='default'
                    disabled={short || busy !== null}
                    loading={busy === badge}
                    onClick={() => doBuy(badge)}
                  >
                    {t(`${page}.buy`, { tokens: tItem('token', badges.tokens) })}
                  </Button>
                )}
              </li>
            );
          })}
        </ul>

        <div className='flex flex-wrap items-center gap-3'>
          {badges.shown && (
            <button type='button' className='theme-link-action' disabled={busy !== null} onClick={() => doShow(null)}>
              {t(`${page}.hide`, {})}
            </button>
          )}
          {badges.enabled && short && (
            <span className='theme-eco-small'>
              {t(`${page}.balance`, { balance: tItem('token', badges.balance) })}{' '}
              <Link to='/account/tokens' className='theme-eco-link'>
                {t(`${page}.earn`, {})}
              </Link>
            </span>
          )}
        </div>
      </div>
    </TitleCard>
  );
}
