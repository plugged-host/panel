import { NumberInputProps } from '@mantine/core';
import NumberInput from '@/elements/input/NumberInput.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const SECONDS_PER_DAY = 24 * 60 * 60;

type Props = Omit<NumberInputProps, 'value' | 'onChange'> & {
  value: number;
  onChange: (seconds: number) => void;
};

export default function TrashRetentionInput({ value, onChange, ...rest }: Props) {
  const { t } = useTranslations();

  return (
    <NumberInput
      min={0}
      max={365}
      decimalScale={2}
      suffix={` ${t('common.elements.trashRetention.days', {})}`}
      value={Math.round((value / SECONDS_PER_DAY) * 100) / 100}
      onChange={(days) => onChange(Math.round(Number(days || 0) * SECONDS_PER_DAY))}
      {...rest}
    />
  );
}
