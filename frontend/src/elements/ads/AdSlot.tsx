// plugged.host: an ad slot and what fills it (docs/features/ads.md#slots)
import { useEffect, useMemo, useRef, useState } from 'react';
import { useLocation } from 'react-router';
import { type AdSlotKind, type AdUnit, adAreaOf, allocate, shuffled, unitFrameUrl, unitSandbox } from '@/lib/ads.ts';
import { useAds } from '@/plugins/useAds.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdSlotsStore } from '@/stores/adSlots.ts';
import AdFrame from './AdFrame.tsx';

let nextSlotId = 1;

/** The units the slots on screen get: no unit twice at once. */
function useAdUnits(): Map<number, AdUnit> {
  const { pathname } = useLocation();
  const { settings, account, shown } = useAds(adAreaOf(pathname));
  const slots = useAdSlotsStore((state) => state.slots);

  // banners of one size take turns from page to page
  const banners = useMemo(() => shuffled(account?.banners ?? [], pathname), [account?.banners, pathname]);

  return useMemo(
    () => (shown ? allocate([...slots.values()], settings.zones, banners, settings.native) : new Map<number, AdUnit>()),
    [shown, slots, settings?.zones, settings?.native, banners],
  );
}

function SponsoredBanner({ unit }: { unit: Extract<AdUnit, { type: 'banner' }> }) {
  const { t } = useTranslations();
  const { banner } = unit;

  return (
    <div className='theme-ad-sponsored' style={{ maxWidth: unit.width }}>
      <span className='theme-ad-label'>{t('elements.ads.sponsored', {})}</span>
      <a href={`/api/ads/banners/${banner.uuid}/click`} target='_blank' rel='sponsored noopener'>
        <img src={banner.imageUrl} alt={banner.advertiser} width={unit.width} height={unit.height} decoding='async' />
      </a>
    </div>
  );
}

/** A slot: measures the width it has and shows the unit it was given, or nothing. */
export function AdSlot({ kind }: { kind: AdSlotKind }) {
  const { pathname } = useLocation();
  const { settings, refreshSeconds } = useAds(adAreaOf(pathname));
  const register = useAdSlotsStore((state) => state.register);
  const unregister = useAdSlotsStore((state) => state.unregister);
  const units = useAdUnits();
  const box = useRef<HTMLDivElement>(null);
  const [id] = useState(() => nextSlotId++);

  useEffect(() => {
    if (!box.current) return;

    const element = box.current;
    const measure = () => register({ id, kind, width: Math.floor(element.clientWidth) });
    measure();

    const observer = new ResizeObserver(measure);
    observer.observe(element);

    return () => {
      observer.disconnect();
      unregister(id);
    };
  }, [register, unregister, id, kind]);

  const unit = units.get(id);

  return (
    <div ref={box} className='theme-ad-slot' data-empty={unit ? undefined : ''}>
      {unit?.type === 'banner' ? (
        <SponsoredBanner unit={unit} />
      ) : unit ? (
        <AdFrame
          src={unitFrameUrl(settings.frameOrigin, unit)}
          sandbox={unitSandbox(settings.frameOrigin)}
          width={unit.type === 'zone' ? unit.width : null}
          height={unit.type === 'zone' ? unit.height : settings.nativeHeight}
          refreshSeconds={refreshSeconds ?? 0}
        />
      ) : null}
    </div>
  );
}

/** What a waiting room carries: the native unit and the zones the page has left. */
export function AdStack() {
  const { pathname } = useLocation();
  const { settings, account, shown } = useAds(adAreaOf(pathname));
  if (!shown) return null;

  const slots = settings.roomBanner
    ? Math.min(
        6,
        settings.zones.length + (account?.banners.filter((banner) => banner.placement === 'slot').length ?? 0),
      )
    : 0;

  return (
    <div className='theme-ad-stack'>
      {settings.roomNative && <AdSlot kind='native' />}
      {Array.from({ length: slots }, (_, index) => (
        <AdSlot key={index} kind='room' />
      ))}
    </div>
  );
}
