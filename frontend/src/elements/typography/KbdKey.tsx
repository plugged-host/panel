import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import classNames from 'classnames';
import { makeComponentHookable } from 'shared';

interface KbdKeyProps {
  children: React.ReactNode;
  className?: string;
  icon?: IconDefinition;
}

function KbdKey({ children, className, icon }: KbdKeyProps) {
  return (
    <div
      className={classNames(
        'inline-flex items-center justify-center w-11 h-8 bg-(--mantine-color-default-hover) border border-(--mantine-color-default-border) rounded-md text-xs font-semibold font-sans text-(--mantine-color-text)',
        className,
      )}
    >
      {icon ? <FontAwesomeIcon icon={icon} size='sm' /> : children}
    </div>
  );
}

export default makeComponentHookable(KbdKey);
