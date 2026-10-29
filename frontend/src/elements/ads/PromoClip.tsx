// plugged.host: a house promo clip in an ad slot (docs/features/promos.md#promo-clips): muted, looping,
// inline, its button over its foot; where a video does not fit, its poster as a banner to the same page
import { useReducedMotion } from '@mantine/hooks';
import { useState } from 'react';
import { type AdUnit } from '@/lib/ads.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

type PromoUnit = Extract<AdUnit, { type: 'promo' }>;

/** The clips the panel knows get the button in the reader's language; another clip keeps its own text. */
const KNOWN: Record<
  string,
  'elements.ads.promo.skipTheWait' | 'elements.ads.promo.goPlus' | 'elements.ads.promo.freeTokens'
> = {
  'skip-the-wait': 'elements.ads.promo.skipTheWait',
  'go-plus': 'elements.ads.promo.goPlus',
  'free-tokens': 'elements.ads.promo.freeTokens',
};

export default function PromoClip({ unit, labelled = true }: { unit: PromoUnit; labelled?: boolean }) {
  const { t } = useTranslations();
  const reducedMotion = useReducedMotion();
  const economy = useGlobalStore((state) => state.settings.economy);
  const [failed, setFailed] = useState(false);
  const { clip } = unit;
  if (failed) return null;

  const name = clip.name === 'go-plus' ? economy?.plusName : economy?.plusplusName;
  const cta = KNOWN[clip.name] ? t(KNOWN[clip.name], { name: name ?? '' }) : clip.cta;
  const base = `/promo/${clip.name}`;

  return (
    <div className='theme-ad-sponsored theme-ad-promo' style={{ maxWidth: unit.width }}>
      {labelled && <span className='theme-ad-label'>{t('elements.ads.sponsored', {})}</span>}
      {unit.video ? (
        <div className='theme-ad-promo-video'>
          <video
            width={unit.width}
            height={unit.height}
            poster={`${base}.jpg`}
            autoPlay={!reducedMotion}
            controls={!!reducedMotion}
            muted
            loop
            playsInline
            preload='metadata'
            aria-label={cta}
          >
            <source src={`${base}.webm`} type='video/webm' />
            {/* the last source failing means none plays: the slot shows nothing */}
            <source src={`${base}.mp4`} type='video/mp4' onError={() => setFailed(true)} />
          </video>
          <a className='theme-ad-promo-cta' href={clip.href} target='_blank' rel='noopener'>
            {cta}
          </a>
        </div>
      ) : (
        <a href={clip.href} target='_blank' rel='noopener'>
          <img
            className='theme-ad-promo-poster'
            src={`${base}.jpg`}
            alt={cta}
            width={unit.width}
            height={unit.height}
            decoding='async'
            onError={() => setFailed(true)}
          />
        </a>
      )}
    </div>
  );
}
