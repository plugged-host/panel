// plugged.host: Admin › Plans and Admin › Dedicated Plans
import { z } from 'zod';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { intlLocale } from '@/lib/languages.ts';
import { adminPlanSchema } from '@/lib/schemas/admin/plans.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export const planBasePath = (dedicated: boolean) => (dedicated ? '/admin/dedicated-plans' : '/admin/plans');

export default ({ plan, dedicated }: { plan: z.infer<typeof adminPlanSchema>; dedicated: boolean }) => {
  const { t, language } = useTranslations();

  return (
    <TableRow>
      <TableData>
        <TableLink to={`${planBasePath(dedicated)}/${plan.uuid}`}>
          <Code>{plan.uuid}</Code>
        </TableLink>
      </TableData>

      <TableData>{plan.name}</TableData>

      {/* a dedicated plan is whole threads of its own (docs/features/dedicated.md) */}
      <TableData>
        {dedicated ? t('pages.admin.plans.table.threadsValue', { threads: String(plan.cpu / 100) }) : `${plan.cpu}%`}
      </TableData>

      <TableData>{bytesToString(mbToBytes(plan.memory))}</TableData>

      <TableData>{bytesToString(mbToBytes(plan.disk))}</TableData>

      {dedicated && (
        <TableData>
          {plan.price === null
            ? '-'
            : new Intl.NumberFormat(intlLocale(language), { style: 'currency', currency: 'EUR' }).format(
                plan.price / 100,
              )}
        </TableData>
      )}

      <TableData>{plan.public ? t('common.yes', {}) : t('common.no', {})}</TableData>

      <TableData>
        <FormattedTimestamp timestamp={plan.created} />
      </TableData>
    </TableRow>
  );
};
