// plugged.host: a pick-one tile in a `theme-choice-grid`, and the titled section of a form made of them
import { ReactNode } from 'react';
import Stack from '@/elements/layout/Stack.tsx';
import Title from '@/elements/typography/Title.tsx';

export default function Choice({
  selected,
  disabled,
  onSelect,
  children,
}: {
  selected: boolean;
  disabled?: boolean;
  onSelect: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type='button'
      className='theme-choice'
      data-selected={selected || undefined}
      aria-pressed={selected}
      disabled={disabled}
      onClick={onSelect}
    >
      {children}
    </button>
  );
}

export function ChoiceSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Stack gap='xs'>
      <Title order={3} fz='md' fw={600}>
        {title}
      </Title>
      {children}
    </Stack>
  );
}
