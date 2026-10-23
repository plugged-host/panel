import { Box, Group, Title } from '@mantine/core';
import classNames from 'classnames';
import type { ReactNode } from 'react';
import { makeComponentHookable } from 'shared';
import Card from '@/elements/data-display/Card.tsx';

export interface TitleCardProps {
  title: string;
  icon?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  titleClassName?: string;
  wrapperClassName?: string;
  iconClassName?: string;
  leftSection?: ReactNode;
  rightSection?: ReactNode;
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
}: TitleCardProps) {
  return (
    // plugged.host: Filament's section, "bg-white/5 rounded-2xl": the heading "px-6 py-4 text-base
    // font-semibold" over a hairline, the content "p-6".
    <Card withBorder p={0} className={className}>
      <Group
        id='title-card-header'
        className={classNames('px-4 py-3 md:px-6 md:py-4', titleClassName)}
        style={{ borderBottom: '1px solid var(--theme-hairline)' }}
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
        <Title order={5} fw={600}>
          {title}
        </Title>
        {rightSection}
      </Group>
      <div className={classNames('p-4 md:p-6 h-full', wrapperClassName)}>{children}</div>
    </Card>
  );
}

export default makeComponentHookable(TitleCard);
