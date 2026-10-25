import { lazy } from 'react';
import type { PierreDiffEditorProps, PierreEditorProps } from '@/elements/editors/PierreEditorImpl.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';

export type {
  PierreCaret,
  PierreCaretMetadata,
  PierreDiffEditorProps,
  PierreDiffOnMount,
  PierreEditorHandle,
  PierreEditorProps,
  PierreFileChangeEvent,
  PierreFileEditor,
  PierreLocalSelection,
  PierreOnMount,
} from '@/elements/editors/PierreEditorImpl.tsx';

const LazyPierreEditor = lazy(() => import('@/elements/editors/PierreEditorImpl.tsx'));
const LazyPierreDiffEditor = lazy(() =>
  import('@/elements/editors/PierreEditorImpl.tsx').then((module) => ({ default: module.PierreDiffEditor })),
);

export function PierreEditor(props: PierreEditorProps) {
  return (
    // plugged.host: the editor stays left to right in Arabic; `contents` leaves the layout alone
    <div dir='ltr' className='contents'>
      <Spinner.Suspense className='w-full h-full'>
        <LazyPierreEditor {...props} />
      </Spinner.Suspense>
    </div>
  );
}

export function PierreDiffEditor(props: PierreDiffEditorProps) {
  return (
    // plugged.host: the editor stays left to right in Arabic; `contents` leaves the layout alone
    <div dir='ltr' className='contents'>
      <Spinner.Suspense className='w-full h-full'>
        <LazyPierreDiffEditor {...props} />
      </Spinner.Suspense>
    </div>
  );
}

export default PierreEditor;
