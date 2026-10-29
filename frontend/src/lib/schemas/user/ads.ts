// plugged.host: whether this account sees ads, and the sponsored banners (shared/src/ads.rs)
import { z } from 'zod';

export const adBannerSchema = z.object({
  uuid: z.string(),
  advertiser: z.string(),
  placement: z.enum(['slot', 'top']),
  imageUrl: z.string(),
  width: z.number(),
  height: z.number(),
});

export const accountAdsSchema = z.object({
  audience: z.enum(['none', 'room', 'member', 'everyone']),
  banners: z.array(adBannerSchema),
});

// the house promo clips, `public/promo/index.json` (docs/features/promos.md#promo-clips)
export const promoClipSchema = z.object({
  name: z.string().regex(/^[a-z0-9-]{1,64}$/),
  cta: z.string().max(64),
  href: z.string().startsWith('/'),
});
