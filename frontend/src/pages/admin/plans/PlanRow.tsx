// plugged.host: Admin › Plans
import { z } from 'zod';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { adminPlanSchema } from '@/lib/schemas/admin/plans.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default ({ plan }: { plan: z.infer<typeof adminPlanSchema> }) => {
  const { t } = useTranslations();

  return (
    <TableRow>
      <TableData>
        <TableLink to={`/admin/plans/${plan.uuid}`}>
          <Code>{plan.uuid}</Code>
        </TableLink>
      </TableData>

      <TableData>{plan.name}</TableData>

      <TableData>{plan.cpu}%</TableData>

      <TableData>{bytesToString(mbToBytes(plan.memory))}</TableData>

      <TableData>{bytesToString(mbToBytes(plan.disk))}</TableData>

      <TableData>{plan.public ? t('common.yes', {}) : t('common.no', {})}</TableData>

      <TableData>
        <FormattedTimestamp timestamp={plan.created} />
      </TableData>
    </TableRow>
  );
};
