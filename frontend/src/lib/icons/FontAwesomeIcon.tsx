/*
 * plugged.host: what `import { FontAwesomeIcon } from '@fortawesome/react-fontawesome'` resolves to
 * everywhere in the panel (vite-plugins/tabler-icons.ts). An icon tablerIcons.ts knows is drawn
 * as Tabler's outline icon, the Pelican panel's set; brands and the rest fall through to the real
 * Font Awesome component, which this module alone imports.
 *
 * The box is Font Awesome's (1.25em wide, centred on the text the same way), so a swapped icon
 * takes the room the solid one took. Its own CSS is the "Tabler icons" block in app.css.
 */
import type { IconDefinition, IconProp } from '@fortawesome/fontawesome-svg-core';
import { type FontAwesomeIconProps, FontAwesomeIcon as RealFontAwesomeIcon } from '@fortawesome/react-fontawesome';
import classNames from 'classnames';
import { forwardRef } from 'react';
import { tablerIcons } from './tablerIcons.ts';

export type { FontAwesomeIconProps };

function tablerIconFor(icon: IconProp) {
  if (!icon || typeof icon !== 'object' || Array.isArray(icon)) return null;

  const definition = icon as IconDefinition;
  if (definition.prefix === 'fab') return null;

  return tablerIcons[definition.iconName] ?? null;
}

export const FontAwesomeIcon = forwardRef<SVGSVGElement, FontAwesomeIconProps>((props, ref) => {
  const TablerIcon = tablerIconFor(props.icon);

  if (!TablerIcon) {
    return <RealFontAwesomeIcon ref={ref} {...props} />;
  }

  const {
    icon: _icon,
    mask: _mask,
    maskId: _maskId,
    transform: _transform,
    symbol: _symbol,
    swapOpacity: _swapOpacity,
    inverse: _inverse,
    titleId: _titleId,
    className,
    size,
    spin,
    spinPulse,
    spinReverse,
    pulse,
    fixedWidth,
    widthAuto,
    listItem,
    pull,
    rotation,
    flip,
    border: _border,
    beat: _beat,
    fade: _fade,
    beatFade: _beatFade,
    bounce: _bounce,
    shake: _shake,
    title,
    ...rest
  } = props;

  return (
    <TablerIcon
      ref={ref}
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
      title={title}
      stroke={1.75}
      className={classNames(
        'theme-icon',
        size && `theme-icon-${size}`,
        (spin || spinPulse || pulse) && 'theme-icon-spin',
        spinReverse && 'theme-icon-spin-reverse',
        fixedWidth && 'theme-icon-fw',
        widthAuto && 'theme-icon-width-auto',
        listItem && 'theme-icon-li',
        pull && `theme-icon-pull-${pull}`,
        rotation && `theme-icon-rotate-${rotation}`,
        flip && `theme-icon-flip-${flip === true ? 'horizontal' : flip}`,
        className,
      )}
      {...(rest as object)}
    />
  );
});

FontAwesomeIcon.displayName = 'FontAwesomeIcon';
