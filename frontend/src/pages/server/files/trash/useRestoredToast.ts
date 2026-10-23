import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import deleteFiles from '@/api/server/files/deleteFiles.ts';
import { isTrashEnabled } from '@/lib/domain/server.ts';
import { createUndoAction } from '@/lib/files/undoableFileMutation.ts';
import { serverTrashEntrySchema } from '@/lib/schemas/server/files.ts';
import { useUndoableToast } from '@/plugins/toast/useUndoableToast.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore, useServerStoreApi } from '@/stores/server.ts';
import { fileManagerUndoScope } from '@/stores/undoHistory.ts';

export function useRestoredToast(onUndone: () => void) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const serverStoreApi = useServerStoreApi();
  const addUndoableToast = useUndoableToast(fileManagerUndoScope(server.uuid));

  return (restored: z.infer<typeof serverTrashEntrySchema>[], undoable = true) => {
    if (restored.length === 0) return;

    addUndoableToast(
      t('pages.server.files.trash.toast.restored', { entries: tItem('file', restored.length) }),
      undoable && isTrashEnabled(server, true)
        ? createUndoAction(
            () =>
              isTrashEnabled(serverStoreApi.getState().server, true)
                ? deleteFiles(
                    server.uuid,
                    '/',
                    restored.map((entry) => entry.path),
                  )
                : Promise.resolve(null),
            (result) => result?.entries.length ?? 0,
            {
              addToast,
              invalidateFilemanager: onUndone,
              cannotUndoMessage: t('pages.server.files.trash.toast.restoreCouldNotBeUndone', {}),
              undoneMessage: (count) =>
                t('pages.server.files.trash.toast.restoreUndone', { files: tItem('file', count) }),
              onError: (msg) => addToast(httpErrorToHuman(msg), 'error'),
            },
          )
        : null,
    );
  };
}
