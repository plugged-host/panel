import { ReactNode } from 'react';
import { AddToast } from '@/providers/contexts/toastContext.ts';

export function createUndoAction<R>(
  reverseCall: () => Promise<R>,
  getCount: (result: R) => number,
  {
    addToast,
    invalidateFilemanager,
    cannotUndoMessage,
    undoneMessage,
    onError,
  }: {
    addToast: AddToast;
    invalidateFilemanager: () => void;
    cannotUndoMessage: ReactNode;
    undoneMessage: ReactNode | ((count: number) => ReactNode);
    onError: (err: unknown) => void;
  },
): () => Promise<void> {
  return () =>
    reverseCall()
      .then((result) => {
        const count = getCount(result);
        if (count < 1) {
          addToast(cannotUndoMessage, 'error');
          return;
        }

        addToast(typeof undoneMessage === 'function' ? undoneMessage(count) : undoneMessage, 'success');
        invalidateFilemanager();
      })
      .catch(onError);
}
