import { faMagnifyingGlass } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import Button from '@/elements/buttons/Button.tsx';
import Kbd from '@/elements/typography/Kbd.tsx';
import { getShortcutDefinition } from '@/lib/quickActions/coreShortcuts.tsx';
import { useShortcutOverrides } from '@/lib/quickActions/shortcutOverrides.ts';
import { effectiveBinding, ModifierKey, ShortcutBinding } from '@/lib/quickActions/shortcuts.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useQuickActionsStore } from '@/stores/quickActions.ts';

const MODIFIER_ORDER: ModifierKey[] = ['ctrlOrMeta', 'ctrl', 'meta', 'alt', 'shift'];

function modifierLabel(modifier: ModifierKey, isMac: boolean): string {
  switch (modifier) {
    case 'ctrlOrMeta':
      return isMac ? 'Cmd' : 'Ctrl';
    case 'meta':
      return 'Cmd';
    case 'alt':
      return isMac ? 'Opt' : 'Alt';
    case 'ctrl':
      return 'Ctrl';
    case 'shift':
      return 'Shift';
  }
}

function formatBindingCompact(binding: ShortcutBinding, isMac: boolean): string {
  const parts = MODIFIER_ORDER.filter((m) => binding.modifiers.includes(m)).map((m) => modifierLabel(m, isMac));
  parts.push(binding.key === ' ' ? 'Space' : binding.key.length === 1 ? binding.key.toUpperCase() : binding.key);
  return parts.join('+');
}

export default function QuickActionsTrigger() {
  const { t } = useTranslations();
  const setOpen = useQuickActionsStore((state) => state.setOpen);
  const overrides = useShortcutOverrides();

  const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
  const definition = getShortcutDefinition('general.quickActions');
  const binding = definition ? effectiveBinding(definition, overrides) : null;
  const hint = binding ? formatBindingCompact(binding, isMac) : null;

  return (
    // plugged.host: a sidebar row like Theme v2's notification button, "transparent until hover"
    <Button
      variant='subtle'
      color='gray'
      fullWidth
      className='theme-sidebar-search'
      leftSection={<FontAwesomeIcon icon={faMagnifyingGlass} />}
      rightSection={hint ? <Kbd size='xs'>{hint}</Kbd> : undefined}
      onClick={() => setOpen(true)}
      styles={{ label: { width: '100%', color: 'var(--theme-text-nav)' } }}
    >
      {t('elements.quickActions.trigger', {})}
    </Button>
  );
}
