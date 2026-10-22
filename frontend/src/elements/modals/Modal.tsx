import { Group, Modal as MantineModal, ModalProps } from '@mantine/core';
import { forwardRef } from 'react';
import { makeComponentHookable } from 'shared';

export const Modal = makeComponentHookable(
  forwardRef<HTMLDivElement, ModalProps>(({ children, className, ...rest }, ref) => {
    return (
      <MantineModal
        centered
        // plugged.host: Filament's overlay, "bg-gray-950/75" without a blur.
        overlayProps={{
          backgroundOpacity: 0.75,
          color: '#0b0d10',
        }}
        removeScrollProps={{ removeScrollBar: false }}
        ref={ref}
        className={className}
        {...rest}
      >
        {children}
      </MantineModal>
    );
  }),
  {
    Stack: MantineModal.Stack,
  },
);

export const ModalFooter = makeComponentHookable(({ children }: { children: React.ReactNode }) => {
  return (
    <Group mt='lg' justify='flex-end'>
      {children}
    </Group>
  );
});
