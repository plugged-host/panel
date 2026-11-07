import { FileInputProps, FileInput as MantineFileInput } from '@mantine/core';
import { ForwardedRef, forwardRef, ReactElement, Ref } from 'react';
import { makeComponentHookable } from 'shared';

function FileInputInner<Multiple extends boolean = false>(
  { className, ...rest }: FileInputProps<Multiple>,
  ref: ForwardedRef<HTMLButtonElement>,
) {
  return (
    <MantineFileInput
      ref={ref}
      className={className}
      placeholder={typeof rest.label === 'string' ? rest.label : undefined}
      {...rest}
    />
  );
}

// plugged.host: generic so `multiple` narrows value/onChange to File[] at the call site.
const FileInput = forwardRef(FileInputInner) as <Multiple extends boolean = false>(
  props: FileInputProps<Multiple> & { ref?: Ref<HTMLButtonElement> },
) => ReactElement;

export default makeComponentHookable(FileInput) as typeof FileInput;
