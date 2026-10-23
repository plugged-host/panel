import { Title } from '@mantine/core';
import { Children, ReactNode } from 'react';
import { makeComponentHookable } from 'shared';
import Card from '@/elements/data-display/Card.tsx';

function ScreenBlock({ title, content, children }: { title: string; content: string; children?: ReactNode }) {
  return (
    <div className='flex items-center justify-center lg:mt-6 mt-2'>
      <Card className='w-full max-w-md text-center'>
        <Title order={2}>{title}</Title>
        <div className='text-sm text-(--mantine-color-dimmed) mt-2'>{content}</div>
        {Children.toArray(children).length > 0 && (
          <div className='flex flex-row flex-wrap items-center justify-center gap-3 mt-4'>{children}</div>
        )}
      </Card>
    </div>
  );
}

export default makeComponentHookable(ScreenBlock);
