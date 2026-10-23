import classNames from 'classnames';
import { ComponentProps } from 'react';
import { NavLink } from 'react-router';

export default function TableLink({ className, children, ...rest }: ComponentProps<typeof NavLink>) {
  return (
    <NavLink className={classNames('text-(--mantine-color-anchor) hover:underline', className)} {...rest}>
      {children}
    </NavLink>
  );
}
