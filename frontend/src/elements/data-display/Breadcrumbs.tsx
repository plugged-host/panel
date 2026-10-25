import { BreadcrumbsProps, Breadcrumbs as MantineBreadcrumbs } from '@mantine/core';
import { forwardRef } from 'react';
import { makeComponentHookable } from 'shared';

const Breadcrumbs = forwardRef<HTMLDivElement, BreadcrumbsProps>(({ ...rest }, ref) => {
  // plugged.host: the panel's breadcrumbs are paths, which read left to right in Arabic too
  return <MantineBreadcrumbs ref={ref} dir='ltr' {...rest} />;
});

export default makeComponentHookable(Breadcrumbs);
