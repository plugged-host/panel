import { faDownload } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { forwardRef, memo } from 'react';
import { z } from 'zod';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import { TableData, TableRow, TableSelectionCell } from '@/elements/data-display/Table.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import { adminEggRepositoryEggSchema } from '@/lib/schemas/admin/eggRepositories.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

interface EggRepositoryEggRowProps {
  egg: z.infer<typeof adminEggRepositoryEggSchema>;
  isSelected: boolean;
  onSelectionChange: (selected: boolean, range?: boolean) => void;
  onOpen: () => void;
  onInstall: () => void;
}

const EggRepositoryEggRow = memo(
  forwardRef<HTMLTableRowElement, EggRepositoryEggRowProps>(function EggRepositoryEggRow(
    { egg, isSelected, onSelectionChange, onOpen, onInstall },
    ref,
  ) {
    const { t } = useTranslations();

    return (
      <TableRow
        bg={isSelected ? 'var(--mantine-color-blue-light)' : undefined}
        ref={ref}
        className='cursor-pointer'
        onClick={onOpen}
      >
        <TableSelectionCell id={egg.uuid} checked={isSelected} onChange={onSelectionChange} />

        <TableData>
          <Code>{egg.path}</Code>
        </TableData>

        <TableData>{egg.exportedEgg.name}</TableData>

        <TableData>{egg.exportedEgg.author}</TableData>

        <TableData>{egg.exportedEgg.description}</TableData>

        <TableData>
          <FormattedTimestamp timestamp={egg.updated} />
        </TableData>

        <TableData>
          <AdminCan action={['eggs.create', 'nests.read']}>
            <Tooltip label={t('common.button.install', {})}>
              <ActionIcon
                variant='subtle'
                onClick={(e) => {
                  e.stopPropagation();
                  onInstall();
                }}
              >
                <FontAwesomeIcon icon={faDownload} />
              </ActionIcon>
            </Tooltip>
          </AdminCan>
        </TableData>
      </TableRow>
    );
  }),
);

export default EggRepositoryEggRow;
