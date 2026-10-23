import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { ReactNode } from 'react';
import { makeComponentHookable } from 'shared';
import Paper from '@/elements/layout/Paper.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';

export interface EmptyStateProps {
  icon: IconDefinition;
  title: string;
  description: ReactNode;
  flush?: boolean;
  children?: ReactNode;
}

// plugged.host: Filament's empty state ("fi-ta-empty-state"): a "text-base font-semibold" heading
// over the description, without the illustration (`icon` is still accepted, for the callers).
function EmptyState({ title, description, flush = false, children }: EmptyStateProps) {
  const content = (
    <>
      <Title order={3} fz='md' fw={600} mb={4}>
        {title}
      </Title>
      <Text c='dimmed' mb='md'>
        {description}
      </Text>
      {children}
    </>
  );

  return flush ? (
    <div style={{ textAlign: 'center', paddingBlock: 'var(--mantine-spacing-lg)' }}>{content}</div>
  ) : (
    <Paper withBorder p='xl' radius='md' style={{ textAlign: 'center' }}>
      {content}
    </Paper>
  );
}

export default makeComponentHookable(EmptyState);
