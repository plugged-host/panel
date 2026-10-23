// plugged.host: sponsored banners, sold by the panel's owner (shared/src/models/ad_banner.rs)
import { z } from 'zod';

export const adminAdBannerSchema = z.looseObject({
  uuid: z.string(),
  advertiser: z.string().min(1).max(255),
  link: z.url({ protocol: /^https$/ }).max(1024),
  placement: z.enum(['slot', 'top']),
  imageUrl: z.string().nullable(),
  width: z.number(),
  height: z.number(),
  starts: z.coerce.date().nullable(),
  ends: z.coerce.date().nullable(),
  active: z.boolean(),
  live: z.boolean(),
  clicks: z.number(),
  created: z.string(),
});

export const adminAdBannerUpdateSchema = adminAdBannerSchema.pick({
  advertiser: true,
  link: true,
  placement: true,
  starts: true,
  ends: true,
  active: true,
});
