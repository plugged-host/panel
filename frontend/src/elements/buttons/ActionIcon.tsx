import { ActionIconProps, ActionIcon as MantineActionIcon, PolymorphicComponentProps } from '@mantine/core';
import classNames from 'classnames';
import { forwardRef } from 'react';
import { makeComponentHookable } from 'shared';

const ActionIcon = forwardRef<HTMLButtonElement, PolymorphicComponentProps<'button', ActionIconProps>>(
  ({ className, size, ...rest }, ref) => {
    return (
      <MantineActionIcon
        ref={ref}
        size={size}
        className={classNames(
          className,
          // Filament's icon-button sizes, so a row action is the same in both panels.
          'theme-icon-btn',
          size === 'sm' && 'theme-icon-btn-sm',
          size === 'xs' && 'theme-icon-btn-xs',
        )}
        {...rest}
      />
    );
  },
);

export default makeComponentHookable(ActionIcon);
