// plugged.host: one value of a settings file (docs/features/minecraft.md)
import { z } from 'zod';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import { minecraftConfigFieldSchema } from '@/lib/schemas/server/minecraft.ts';

export type ConfigValue = z.infer<typeof minecraftConfigFieldSchema>['value'];

/** `max-players` → `Max Players`. */
export function humanize(key: string): string {
  return key
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

interface Props {
  field: z.infer<typeof minecraftConfigFieldSchema>;
  value: ConfigValue;
  disabled: boolean;
  onChange: (value: ConfigValue) => void;
}

export default function ConfigField({ field, value, disabled, onChange }: Props) {
  const label = humanize(field.path[field.path.length - 1] ?? '');
  const description = field.path.join('.');

  switch (field.kind) {
    case 'toggle':
      return (
        <Switch
          label={label}
          description={description}
          checked={value === true}
          disabled={disabled}
          onChange={(e) => onChange(e.currentTarget.checked)}
        />
      );
    case 'number':
      return (
        <NumberInput
          label={label}
          description={description}
          value={typeof value === 'number' ? value : String(value ?? '')}
          disabled={disabled}
          onChange={(number) => onChange(typeof number === 'number' ? number : number === '' ? null : Number(number))}
        />
      );
    case 'choice':
      return (
        <Select
          label={label}
          description={description}
          data={field.choices}
          value={typeof value === 'string' ? value : null}
          disabled={disabled}
          onChange={(choice) => choice !== null && onChange(choice)}
        />
      );
    default:
      return (
        <TextInput
          label={label}
          description={description}
          value={value === null ? '' : String(value)}
          disabled={disabled}
          onChange={(e) => onChange(e.currentTarget.value)}
        />
      );
  }
}
