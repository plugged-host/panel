import { memo } from 'react';
import UploadDropOverlay from '@/elements/UploadDropOverlay.tsx';

interface ImportOverlayProps {
  visible: boolean;
  title: string;
  subtitle: string;
}

function ImportOverlay(props: ImportOverlayProps) {
  return <UploadDropOverlay blur {...props} />;
}

export default memo(ImportOverlay);
