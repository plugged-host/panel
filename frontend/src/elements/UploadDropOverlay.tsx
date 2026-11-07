import { faUpload } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import classNames from 'classnames';
import { memo } from 'react';

interface UploadDropOverlayProps {
  visible: boolean;
  title: string;
  subtitle: string;
  blur?: boolean;
}

function UploadDropOverlay({ visible, title, subtitle, blur = false }: UploadDropOverlayProps) {
  if (!visible) return null;

  return (
    <div
      className={classNames('pointer-events-none fixed inset-0 z-100 flex items-center justify-center', 'bg-black/50')}
    >
      <div className='bg-(--mantine-color-body) rounded-lg p-8 border border-(--mantine-color-default-border)'>
        <div className='flex flex-col items-center gap-4'>
          <FontAwesomeIcon icon={faUpload} className='text-4xl text-(--mantine-color-blue-5)' />
          <p className='text-xl font-semibold'>{title}</p>
          <p className='text-sm text-(--mantine-color-dimmed)'>{subtitle}</p>
        </div>
      </div>
    </div>
  );
}

export default memo(UploadDropOverlay);
