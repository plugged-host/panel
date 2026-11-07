/*
 * plugged.host: one Heroicons outline icon from the shared set switched.cloud renders
 * (src/lib/icons/heroIconPaths.ts, generated from its templates/icons/o-*.svg). The wrapper
 * keeps Font Awesome's box through the same .theme-icon classes, so a swapped icon takes the
 * room the solid one took.
 */
import classNames from 'classnames';
import { forwardRef } from 'react';
import { heroIconPaths } from './heroIconPaths.ts';

export interface HeroIconProps {
  name: string;
  className?: string;
  size?: string;
  title?: string;
  spin?: boolean;
  stroke?: number;
}

export const HeroIcon = forwardRef<SVGSVGElement, HeroIconProps>(
  ({ name, className, size, title, spin, stroke = 1.75 }, ref) => {
    const body = heroIconPaths[name];
    if (!body) return null;

    return (
      <svg
        ref={ref}
        viewBox='0 0 24 24'
        fill='none'
        stroke='currentColor'
        strokeWidth={stroke}
        strokeLinecap='round'
        strokeLinejoin='round'
        aria-hidden={title ? undefined : true}
        role={title ? 'img' : undefined}
        className={classNames('theme-icon', size && `theme-icon-${size}`, spin && 'theme-icon-spin', className)}
      >
        {title ? <title>{title}</title> : null}
        <g dangerouslySetInnerHTML={{ __html: body }} />
      </svg>
    );
  },
);

HeroIcon.displayName = 'HeroIcon';
