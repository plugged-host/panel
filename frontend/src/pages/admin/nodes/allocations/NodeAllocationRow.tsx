import { forwardRef, memo } from 'react';
import { z } from 'zod';
import { TableData, TableRow, TableSelectionCell } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import RedactedText from '@/elements/typography/RedactedText.tsx';
import { ObjectSet } from '@/lib/objectSet.ts';
import { adminNodeAllocationSchema } from '@/lib/schemas/admin/nodes.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

interface NodeAllocationRowProps {
  allocation: z.infer<typeof adminNodeAllocationSchema>;
  selectedNodeAllocations: ObjectSet<z.infer<typeof adminNodeAllocationSchema>, 'uuid'>;
  selectedAllMatching: boolean;
  toggleSelectedNodeAllocation: (
    allocation: z.infer<typeof adminNodeAllocationSchema>,
    selected: boolean,
    range?: boolean,
  ) => void;
}

const NodeAllocationRow = memo(
  forwardRef<HTMLTableRowElement, NodeAllocationRowProps>(function NodeAllocationRow(
    { allocation, selectedNodeAllocations, selectedAllMatching, toggleSelectedNodeAllocation },
    ref,
  ) {
    const { t } = useTranslations();

    const isNodeAllocationSelected = selectedAllMatching || selectedNodeAllocations.has(allocation);

    return (
      <TableRow
        bg={isNodeAllocationSelected ? 'var(--mantine-color-blue-light)' : undefined}
        onClick={(e) => {
          if (e.ctrlKey || e.metaKey || e.shiftKey) {
            toggleSelectedNodeAllocation(allocation, true, e.shiftKey);
            return true;
          }

          return false;
        }}
        ref={ref}
      >
        <TableSelectionCell
          id={allocation.uuid}
          checked={isNodeAllocationSelected}
          disabled={selectedAllMatching}
          onChange={(selected, range) => toggleSelectedNodeAllocation(allocation, selected, range)}
        />

        <TableData>
          <Code>{allocation.uuid}</Code>
        </TableData>

        <TableData>
          <Code>
            {allocation.server ? (
              <TableLink to={`/admin/servers/${allocation.server.uuid}`}>{allocation.server.name}</TableLink>
            ) : (
              t('common.na', {})
            )}
          </Code>
        </TableData>

        <TableData>
          <Code>
            <RedactedText value={allocation.ip} />
          </Code>
        </TableData>

        <TableData>
          <Code>{allocation.ipAlias ? <RedactedText value={allocation.ipAlias} /> : t('common.na', {})}</Code>
        </TableData>

        <TableData>
          <Code>{allocation.port}</Code>
        </TableData>

        <TableData>
          <FormattedTimestamp timestamp={allocation.created} />
        </TableData>
      </TableRow>
    );
  }),
);

export default NodeAllocationRow;
