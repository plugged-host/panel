// plugged.host: Admin › Sponsored Banners
import { z } from 'zod';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import { adminAdBannerSchema } from '@/lib/schemas/admin/adBanners.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default ({ banner }: { banner: z.infer<typeof adminAdBannerSchema> }) => {
  const { t } = useTranslations();

  return (
    <TableRow>
      <TableData>
        <TableLink to={`/admin/ad-banners/${banner.uuid}`}>
          <Code>{banner.uuid}</Code>
        </TableLink>
      </TableData>

      <TableData>{banner.advertiser}</TableData>

      <TableData>{t(`pages.admin.adBanners.placement.${banner.placement}`, {})}</TableData>

      <TableData>{banner.imageUrl ? `${banner.width}×${banner.height}` : '-'}</TableData>

      <TableData>{banner.live ? t('common.yes', {}) : t('common.no', {})}</TableData>

      <TableData>{banner.clicks.toLocaleString()}</TableData>

      <TableData>
        <FormattedTimestamp timestamp={banner.created} />
      </TableData>
    </TableRow>
  );
};
