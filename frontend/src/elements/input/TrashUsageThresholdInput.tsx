import { NumberInputProps } from '@mantine/core';
import NumberInput from '@/elements/input/NumberInput.tsx';

type Props = Omit<NumberInputProps, 'value' | 'onChange'> & {
  value: number;
  onChange: (threshold: number) => void;
};

export default function TrashUsageThresholdInput({ value, onChange, ...rest }: Props) {
  return (
    <NumberInput
      min={0}
      max={100}
      allowDecimal={false}
      suffix=' %'
      value={value}
      onChange={(threshold) => onChange(Number(threshold || 0))}
      {...rest}
    />
  );
}
