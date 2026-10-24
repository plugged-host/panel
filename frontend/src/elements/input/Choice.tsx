// plugged.host: a pick-one card in a `theme-choice-grid`, the Pelican panel's CardSelect
// (plugged-host-servers card-select.blade.php), and the titled section of a form made of them
import classNames from 'classnames';
import { ReactNode } from 'react';
import Stack from '@/elements/layout/Stack.tsx';
import Title from '@/elements/typography/Title.tsx';

export default function Choice({
  selected,
  disabled,
  locked,
  radio,
  centered,
  media,
  corner,
  onSelect,
  children,
}: {
  selected: boolean;
  /** It cannot be taken: dimmed. */
  disabled?: boolean;
  /** It cannot be taken until it is unlocked: not dimmed and without a radio, so it may hold its own link. */
  locked?: boolean;
  /** A radio dot before the content (locations, plans). */
  radio?: boolean;
  /** The content centred under `media` (games). */
  centered?: boolean;
  /** A picture before the content: a game's icon, a location's flag. */
  media?: ReactNode;
  /** Something pinned to the top corner (a location's latency). */
  corner?: ReactNode;
  onSelect: () => void;
  children: ReactNode;
}) {
  const className = classNames('theme-choice', {
    'theme-choice-row': radio,
    'theme-choice-centered': centered,
    'theme-choice-cornered': !!corner,
  });
  const content = (
    <>
      {corner}
      {radio && !locked && <span className='theme-choice-radio' aria-hidden />}
      {media}
      {radio || centered || media ? <span className='theme-choice-body'>{children}</span> : children}
    </>
  );

  if (locked) {
    return (
      <div className={className} data-locked>
        {content}
      </div>
    );
  }

  return (
    <button
      type='button'
      className={className}
      data-selected={selected || undefined}
      aria-pressed={selected}
      disabled={disabled}
      onClick={onSelect}
    >
      {content}
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
