import { forwardRef } from 'react';
import { z } from 'zod';
import { TableData, TableRow, TableSelectionCell } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import { adminDatabaseAgentHostSchema } from '@/lib/schemas/admin/databaseAgentHosts.ts';
import DatabaseAgentHostDeploymentBadge from './DatabaseAgentHostDeploymentBadge.tsx';
import DatabaseAgentHostHealthIcon from './DatabaseAgentHostHealthIcon.tsx';

interface DatabaseAgentHostRowProps {
  databaseAgentHost: z.infer<typeof adminDatabaseAgentHostSchema>;
  isSelected?: boolean;
  onSelectionChange?: (selected: boolean, range?: boolean) => void;
}

const DatabaseAgentHostRow = forwardRef<HTMLTableRowElement, DatabaseAgentHostRowProps>(function DatabaseAgentHostRow(
  { databaseAgentHost, isSelected = false, onSelectionChange },
  ref,
) {
  return (
    <TableRow
      bg={isSelected ? 'var(--mantine-color-blue-light)' : undefined}
      onClick={(e) => {
        if (e.ctrlKey || e.metaKey || e.shiftKey) {
          onSelectionChange?.(true, e.shiftKey);
          return true;
        }
        return false;
      }}
      ref={ref}
    >
      {onSelectionChange !== undefined && (
        <TableSelectionCell id={databaseAgentHost.uuid} checked={isSelected} onChange={onSelectionChange} />
      )}

      <TableData>
        <DatabaseAgentHostHealthIcon databaseAgentHost={databaseAgentHost} />
      </TableData>

      <TableData>
        <TableLink to={`/admin/database-agent-hosts/${databaseAgentHost.uuid}`}>
          <Code>{databaseAgentHost.uuid}</Code>
        </TableLink>
      </TableData>

      <TableData>
        <span className='flex gap-2 items-center'>
          {databaseAgentHost.name}&nbsp;
          <DatabaseAgentHostDeploymentBadge databaseAgentHost={databaseAgentHost} />
        </span>
      </TableData>

      <TableData>
        <FormattedTimestamp timestamp={databaseAgentHost.created} />
      </TableData>
    </TableRow>
  );
});

export default DatabaseAgentHostRow;
