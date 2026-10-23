import { forwardRef } from 'react';
import { z } from 'zod';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow, TableSelectionCell } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import Group from '@/elements/layout/Group.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import RedactedText from '@/elements/typography/RedactedText.tsx';
import { databaseAgentTypeLabelMapping } from '@/lib/enums.ts';
import { adminServerDatabaseAgentSchema } from '@/lib/schemas/admin/servers.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

interface DatabaseAgentTemplateInstanceRowProps {
  databaseAgent: z.infer<typeof adminServerDatabaseAgentSchema>;
  isSelected?: boolean;
  onSelectionChange?: (selected: boolean, range?: boolean) => void;
}

const DatabaseAgentTemplateInstanceRow = forwardRef<HTMLTableRowElement, DatabaseAgentTemplateInstanceRowProps>(
  function DatabaseAgentTemplateInstanceRow({ databaseAgent, isSelected = false, onSelectionChange }, ref) {
    const { t } = useTranslations();
    const host = databaseAgent.host
      ? `${databaseAgent.host}${databaseAgent.port ? `:${databaseAgent.port}` : ''}`
      : null;

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
        <TableSelectionCell
          id={databaseAgent.uuid}
          checked={isSelected}
          onChange={(selected, range) => onSelectionChange?.(selected, range)}
        />

        <TableData>{databaseAgent.name}</TableData>

        <TableData>
          <TableLink to={`/admin/servers/${databaseAgent.server.uuid}`}>
            <Code>{databaseAgent.server.name}</Code>
          </TableLink>
        </TableData>

        <TableData>{databaseAgentTypeLabelMapping[databaseAgent.type]}</TableData>

        <TableData>
          {host ? (
            <CopyOnClick content={host}>
              <Code>
                <RedactedText value={host} />
              </Code>
            </CopyOnClick>
          ) : null}
        </TableData>

        <TableData>
          <Group gap='xs' wrap='nowrap'>
            {databaseAgent.templateVersion !== null && <Code>v{databaseAgent.templateVersion}</Code>}
            {databaseAgent.updateAvailable && (
              <Badge color='yellow'>{t('pages.admin.databaseAgentTemplates.tabs.instances.page.outdated', {})}</Badge>
            )}
          </Group>
        </TableData>

        <TableData>
          <FormattedTimestamp timestamp={databaseAgent.created} />
        </TableData>
      </TableRow>
    );
  },
);

export default DatabaseAgentTemplateInstanceRow;
