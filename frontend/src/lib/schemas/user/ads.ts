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
  audience: z.enum(['none', 'member', 'everyone']),
  banners: z.array(adBannerSchema),
});
