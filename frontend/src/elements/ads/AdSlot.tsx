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
    () =>
      shown
        ? allocate([...slots.values()], settings.zones, banners, settings.native, settings.maxUnitsPerPage)
        : new Map<number, AdUnit>(),
    [shown, slots, settings?.zones, settings?.native, settings?.maxUnitsPerPage, banners],
  );
}

/** A sponsored banner; in a waiting page's stack the stack's own caption labels it. */
function SponsoredBanner({ unit, labelled = true }: { unit: Extract<AdUnit, { type: 'banner' }>; labelled?: boolean }) {
  const { t } = useTranslations();
  const { banner } = unit;

  return (
    <div className='theme-ad-sponsored' style={{ maxWidth: unit.width }}>
      {labelled && <span className='theme-ad-label'>{t('elements.ads.sponsored', {})}</span>}
      <a href={`/api/ads/banners/${banner.uuid}/click`} target='_blank' rel='sponsored noopener'>
        <img src={banner.imageUrl} alt={banner.advertiser} width={unit.width} height={unit.height} decoding='async' />
      </a>
    </div>
  );
}

/**
 * A slot: measures the width it has and shows the unit it was given, or nothing. Outside the
 * waiting rooms (`room`) it is not there for the first membership, which sees no banners.
 */
export function AdSlot({ room = false, ...props }: { kind: AdSlotKind; labelled?: boolean; room?: boolean }) {
  const { pathname } = useLocation();
  const { pageShown } = useAds(adAreaOf(pathname));
  if (!room && !pageShown) return null;

  return <Slot {...props} />;
}

function Slot({ kind, labelled = true }: { kind: AdSlotKind; labelled?: boolean }) {
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
        <SponsoredBanner unit={unit} labelled={labelled} />
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

/** The room slots the first part of a waiting page's stack takes, under its status card. */
const ROOM_SLOTS_ABOVE = 2;

/**
 * What a waiting page carries, in two parts like the Pelican panel's: `above` the status card's
 * first two units (the widest go first), `below` the rest and then the native unit. Each part has
 * one "Sponsored" caption over a grid of two columns on a wide screen.
 */
export function AdStack({ part }: { part: 'above' | 'below' }) {
  const { t } = useTranslations();
  const { pathname } = useLocation();
  const { settings, account, shown } = useAds(adAreaOf(pathname));
  if (!shown) return null;

  const slots = settings.roomBanner
    ? Math.min(
        6,
        settings.zones.length + (account?.banners.filter((banner) => banner.placement === 'slot').length ?? 0),
      )
    : 0;
  const room = part === 'above' ? Math.min(slots, ROOM_SLOTS_ABOVE) : Math.max(0, slots - ROOM_SLOTS_ABOVE);
  const native = part === 'below' && settings.roomNative;
  if (room === 0 && !native) return null;

  return (
    <div className='theme-ad-stack'>
      <span className='theme-ad-stack-label'>{t('elements.ads.sponsored', {})}</span>
      <div className='theme-ad-stack-grid'>
        {Array.from({ length: room }, (_, index) => (
          <AdSlot key={index} kind='room' labelled={false} room />
        ))}
        {native && <AdSlot kind='native' labelled={false} room />}
      </div>
    </div>
  );
}
