// plugged.host: the head of neo's chat: its name, where it is, and icon buttons (the Pelican panel's
// ".neo-head"); every icon button has its label as tooltip and accessible name
import { faArrowLeft, faChevronDown, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import classNames from 'classnames';
import { ReactNode } from 'react';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { useNeoName } from '@/plugins/useNeo.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useNeoStore } from '@/stores/neo.ts';

export function NeoIconButton({
  icon,
  label,
  onClick,
  disabled,
  className,
}: {
  icon: IconDefinition;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <Tooltip label={label}>
      <button
        type='button'
        className={classNames('theme-neo-iconbtn', className)}
        onClick={onClick}
        disabled={disabled}
        aria-label={label}
      >
        <FontAwesomeIcon icon={icon} />
      </button>
    </Tooltip>
  );
}

/** The head: `back` leads back to the chat; `children` are this view's own buttons, before minimize. */
export default function NeoHead({ sub, back, children }: { sub: string; back?: boolean; children?: ReactNode }) {
  const { t } = useTranslations();
  const name = useNeoName();
  const setOpen = useNeoStore((state) => state.setOpen);
  const setView = useNeoStore((state) => state.setView);

  return (
    <div className='theme-neo-head'>
      {back && (
        <NeoIconButton
          icon={faArrowLeft}
          label={t('elements.neo.button.back', {})}
          onClick={() => setView('chat')}
          className='theme-neo-back'
        />
      )}
      <div className='theme-neo-head-text'>
        <div className='theme-neo-title'>{name}</div>
        <div className='theme-neo-sub'>{sub}</div>
      </div>
      {children}
      <NeoIconButton
        icon={faChevronDown}
        label={t('elements.neo.button.minimize', { name })}
        onClick={() => setOpen(false)}
      />
    </div>
  );
}
