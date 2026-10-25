import { Box, Group, Title } from '@mantine/core';
import classNames from 'classnames';
import type { ReactNode } from 'react';
import { makeComponentHookable } from 'shared';
import Card from '@/elements/data-display/Card.tsx';

export interface TitleCardProps {
  title: string;
  icon?: React.ReactNode;
  children?: React.ReactNode; // plugged.host: optional, a section may be only its heading
  className?: string;
  titleClassName?: string;
  wrapperClassName?: string;
  iconClassName?: string;
  leftSection?: ReactNode;
  rightSection?: ReactNode;
  description?: ReactNode; // plugged.host: Filament's section description, under the title
  footer?: ReactNode; // plugged.host: Filament's section footer, its actions end-aligned over a hairline
  danger?: boolean; // plugged.host: Theme v2's red-edged Danger Zone section
  id?: string; // plugged.host
}

function TitleCard({
  title,
  icon,
  children,
  className,
  titleClassName,
  iconClassName,
  wrapperClassName,
  leftSection,
  rightSection,
  description, // plugged.host
  footer, // plugged.host
  danger, // plugged.host
  id, // plugged.host
}: TitleCardProps) {
  const hasBody = children !== undefined; // plugged.host: a section may be only its heading

  return (
    // plugged.host: Filament's section, "bg-white/5 rounded-2xl": the heading "px-6 py-4 text-base
    // font-semibold" over a hairline, the content "p-6". plugged.host: `id` and the danger section's class.
    <Card withBorder p={0} id={id} className={classNames(className, danger && 'theme-section-danger')}>
      <Group
        id='title-card-header'
        className={classNames('px-4 py-3 md:px-6 md:py-4', titleClassName)}
        style={hasBody ? { borderBottom: '1px solid var(--theme-hairline)' } : undefined} // plugged.host
        gap='sm'
      >
        {leftSection}
        {icon && (
          <Box
            id='title-card-icon'
            className={classNames(
              'bg-(--mantine-color-default-hover) light:bg-(--mantine-color-gray-2)',
              iconClassName,
            )}
            style={{
              width: 28,
              height: 28,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              borderRadius: 6,
            }}
          >
            {icon}
          </Box>
        )}
        {/* plugged.host: the description sits under the title, as in Filament's section heading */}
        {description ? (
          <div className='min-w-0 flex-1'>
            <Title order={5} fw={600}>
              {title}
            </Title>
            <p className='theme-section-description'>{description}</p>
          </div>
        ) : (
          <Title order={5} fw={600}>
            {title}
          </Title>
        )}
        {rightSection}
      </Group>
      {hasBody && ( // plugged.host
        <div className={classNames('p-4 md:p-6 h-full', wrapperClassName)}>{children}</div>
      )}
      {/* plugged.host */}
      {footer && <div className='theme-section-footer'>{footer}</div>}
    </Card>
  );
}

export default makeComponentHookable(TitleCard);
