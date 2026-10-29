// plugged.host: the membership popup (docs/features/promos.md#the-popup): a card that slides up at the
// window's end corner (a sheet on phones) for an account without a membership that sees ads, with the
// membership it is closest to, its perks one by one and how far its tokens are; at most once per the
// settings' hours (the panel keeps when it was shown), never on its first day, the membership or tokens
// pages, a waiting page or while the welcome tour runs.
import { useReducedMotion } from '@mantine/hooks';
import { CSSProperties, useEffect, useId, useState } from 'react';
import { createPortal } from 'react-dom';
import { useLocation, useNavigate } from 'react-router';
import { z } from 'zod';
import claimPromo from '@/api/me/economy/claimPromo.ts';
import Button from '@/elements/buttons/Button.tsx';
import { isAdmin } from '@/lib/auth/permissions.ts';
import { userPromoPerkSchema, userPromoSchema } from '@/lib/schemas/user/promo.ts';
import { useAccountAds } from '@/plugins/useAds.ts';
import { useWelcomeTour } from '@/plugins/useWelcomeTour.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type Promo = z.infer<typeof userPromoSchema>;
type Perk = z.infer<typeof userPromoPerkSchema>;

const DAY_MS = 24 * 60 * 60 * 1000;
// a moment after the page settles, so it never covers what the person came to do
const DELAY_MS = 8_000;
const CLOSE_MS = 200;
// one claim per page load: the panel decides whether it is due
let asked = false;

/** Where the popup never comes up. */
function quietPath(pathname: string): boolean {
  return (
    pathname.startsWith('/account/membership') ||
    pathname.startsWith('/account/tokens') ||
    pathname.startsWith('/admin') ||
    pathname.startsWith('/auth') ||
    /^\/server\/[^/]+\/(queue|waiting)/.test(pathname)
  );
}

/** A number counting up to `target` over `ms` (at once with reduced motion). */
function useCountUp(target: number, ms: number, still: boolean): number {
  const [value, setValue] = useState(still ? target : 0);

  useEffect(() => {
    if (still) {
      setValue(target);
      return;
    }

    let frame = 0;
    const start = performance.now();
    const tick = (now: number) => {
      const progress = Math.min(1, (now - start) / ms);
      // ease out: fast first, settling on the number
      setValue(Math.round(target * (1 - (1 - progress) ** 3)));
      if (progress < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);

    return () => cancelAnimationFrame(frame);
  }, [target, ms, still]);

  return value;
}

function perkText(t: ReturnType<typeof useTranslations>['t'], perk: Perk, name: string): string {
  const base = 'elements.promo.perks';

  switch (perk.id) {
    case 'locations':
      return t(`${base}.locations`, { name });
    case 'queue':
      return perk.first ? t(`${base}.queueFirst`, {}) : t(`${base}.queueAhead`, {});
    case 'burst_cores':
      return t(`${base}.burst`, { percent: perk.cores * 100 });
    case 'no_ads':
      return t(`${base}.noAds`, {});
    case 'fewer_ads':
      return t(`${base}.fewerAds`, {});
    case 'panel_access':
      return t(`${base}.panelAccess`, {});
    case 'neo':
      return t(`${base}.neo`, { percent: perk.percent });
    case 'faster_backups':
      return t(`${base}.fasterBackups`, {});
  }
}

function PromoCard({ promo, onClose }: { promo: Promo; onClose: () => void }) {
  const { t, tItem } = useTranslations();
  const navigate = useNavigate();
  const reducedMotion = !!useReducedMotion();
  const titleId = useId();
  const [closing, setClosing] = useState(false);

  const enough = promo.tokens >= promo.cost;
  const shown = useCountUp(Math.min(promo.tokens, promo.cost), 900, reducedMotion);
  const percent = promo.cost > 0 ? Math.min(100, Math.round((promo.tokens / promo.cost) * 100)) : 100;

  const close = (then?: () => void) => {
    if (closing) return;
    setClosing(true);
    setTimeout(
      () => {
        onClose();
        then?.();
      },
      reducedMotion ? 0 : CLOSE_MS,
    );
  };

  // Escape is "Not now"
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  return createPortal(
    <section
      className='theme-promo'
      role='dialog'
      aria-modal='false'
      aria-labelledby={titleId}
      data-closing={closing || undefined}
    >
      <div className='flex flex-col gap-1'>
        <h2 id={titleId} className='theme-eco-heading'>
          {t('elements.promo.title', { name: promo.name })}
        </h2>
        <p className='theme-eco-small'>
          {promo.fullCost > promo.cost && <s className='me-1'>{tItem('token', promo.fullCost)}</s>}
          {t('elements.promo.price', { tokens: tItem('token', promo.cost), days: promo.days })}
        </p>
      </div>

      <ul className='theme-promo-perks'>
        {promo.perks.map((perk, index) => (
          <li key={perk.id} style={{ '--i': index } as CSSProperties}>
            {perkText(t, perk, promo.name)}
          </li>
        ))}
      </ul>

      {enough ? (
        <p className='theme-promo-enough'>{t('elements.promo.enough', { name: promo.name })}</p>
      ) : (
        <div className='flex flex-col gap-2'>
          <div className='theme-promo-progress-head'>
            <span className='theme-eco-small'>{t('elements.promo.progressLabel', {})}</span>
            <span className='theme-promo-count'>
              {t('elements.promo.progress', { have: shown, cost: tItem('token', promo.cost) })}
            </span>
          </div>
          <div
            className='theme-promo-track'
            role='progressbar'
            aria-valuemin={0}
            aria-valuemax={promo.cost}
            aria-valuenow={Math.min(promo.tokens, promo.cost)}
          >
            <div className='theme-promo-fill' style={{ '--p': `${percent}%` } as CSSProperties} />
          </div>
        </div>
      )}

      <div className='theme-promo-actions'>
        <Button onClick={() => close(() => navigate('/account/membership'))}>
          {t('elements.promo.get', { name: promo.name })}
        </Button>
        <Button variant='subtle' color='gray' onClick={() => close()}>
          {t('elements.promo.notNow', {})}
        </Button>
      </div>
    </section>,
    document.body,
  );
}

export default function PromoPopup() {
  const { pathname } = useLocation();
  const { user, impersonating } = useAuth();
  const ads = useGlobalStore((state) => state.settings.ads);
  const economy = useGlobalStore((state) => state.settings.economy);
  const { data: account } = useAccountAds();
  const { tour } = useWelcomeTour();
  const [promo, setPromo] = useState<Promo | null>(null);

  const touring = !!tour?.panelActive || (!!tour?.serverActive && pathname.startsWith('/server/'));
  // what the browser can tell; the panel checks it all again (shared/src/promos.rs)
  const eligible =
    !asked &&
    !!user &&
    !user.suspended &&
    !impersonating &&
    !isAdmin(user) &&
    !!ads?.enabled &&
    !!economy?.membershipEnabled &&
    !!economy.tokensEnabled &&
    account?.audience === 'everyone' &&
    Date.now() - user.created.getTime() >= DAY_MS &&
    !quietPath(pathname) &&
    !touring;

  useEffect(() => {
    if (!eligible) return;

    const timer = setTimeout(() => {
      if (asked) return;
      asked = true;
      claimPromo()
        .then((next) => setPromo(next))
        .catch(() => undefined);
    }, DELAY_MS);

    return () => clearTimeout(timer);
  }, [eligible]);

  // the tour started, or the person went to the membership page by themselves: it goes away
  useEffect(() => {
    if (promo && (touring || quietPath(pathname))) setPromo(null);
  }, [promo, touring, pathname]);

  if (!promo) return null;

  return <PromoCard promo={promo} onClose={() => setPromo(null)} />;
}
