import { MouseEvent as ReactMouseEvent, useEffect } from 'react';
import { z } from 'zod';
import { serverBackupSchema } from '@/lib/schemas/server/backups.ts';
import { useTableSelection } from '@/plugins/selection/useTableSelection.ts';

interface UseBackupSelectionOptions {
  scope: string;
  activeScope?: string | null;
  setActiveScope?: (scope: string | null) => void;
  items?: z.infer<typeof serverBackupSchema>[];
  enabled?: boolean;
}

/** Only the active backup table retains its selection, preventing overlapping action bars. */
export function useBackupSelection({
  scope,
  activeScope,
  setActiveScope,
  items,
  enabled = true,
}: UseBackupSelectionOptions) {
  const selection = useTableSelection({ items, shortcuts: enabled && activeScope === scope });
  const { selected, clear, toggle, selectAll } = selection;

  const activateScope = () => {
    if (enabled) setActiveScope?.(scope);
  };

  useEffect(() => {
    if ((!enabled || activeScope !== scope) && selected.size > 0) {
      clear();
    }
  }, [enabled, activeScope, scope, selected.size, clear]);

  return {
    ...selection,
    scopeProps: {
      onPointerDownCapture: activateScope,
      onFocusCapture: activateScope,
    },
    selectionAreaProps: {
      ...selection.selectionAreaProps,
      onSelectedStart: (event: ReactMouseEvent | MouseEvent) => {
        activateScope();
        selection.selectionAreaProps.onSelectedStart(event);
      },
    },
    toggle: (backup: z.infer<typeof serverBackupSchema>, isSelected: boolean, range?: boolean) => {
      activateScope();
      toggle(backup, isSelected, range);
    },
    selectAll: () => {
      activateScope();
      selectAll();
    },
  };
}
