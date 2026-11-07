import { Button as MantineButton, ButtonProps as MantineButtonProps } from '@mantine/core';
import classNames from 'classnames';
import { forwardRef, MouseEvent as ReactMouseEvent } from 'react';
import { makeComponentHookable } from 'shared';

export interface ButtonProps extends Omit<MantineButtonProps, 'onClick'> {
  type?: 'button' | 'submit' | 'reset';
  onClick?: (event: ReactMouseEvent<HTMLButtonElement, MouseEvent>) => void | Promise<void>;
}

const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ children, className, type = 'button', onClick, loading, disabled, variant = 'filled', size, ...rest }, ref) => {
    return (
      <MantineButton
        ref={ref}
        type={type}
        className={classNames(
          className,
          // plugged.host: Filament's button heights (2.75rem default, 2.125rem small, 1.75rem
          // extra small) so a button is the same size in both panels; see ".theme-btn" in app.css.
          'theme-btn',
          size === 'sm' && 'theme-btn-sm',
          size === 'xs' && 'theme-btn-xs',
        )}
        size={size}
        onClick={onClick}
        loading={disabled ? false : loading}
        disabled={disabled}
        variant={variant}
        {...rest}
        style={{
          cursor: loading ? 'wait' : undefined,
          // plugged.host: Filament's buttons are "font-semibold"; a disabled one only fades.
          fontWeight: 600,
          ...rest.style,
        }}
      >
        {children}
      </MantineButton>
    );
  },
);

export default makeComponentHookable(Button);
