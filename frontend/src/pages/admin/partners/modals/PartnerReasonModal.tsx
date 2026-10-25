// plugged.host: Admin › Partners: rejecting, revoking or pausing, with a reason the applicant sees.
import { DefaultMantineColor, ModalProps } from '@mantine/core';
import { useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Props = Omit<ModalProps, 'children'> & {
  description: string;
  confirm: string;
  confirmColor: DefaultMantineColor;
  /** A reason must be given (rejecting, revoking); pausing may go without. */
  required: boolean;
  onConfirmed: (reason: string) => Promise<void>;
};

export default function PartnerReasonModal({
  description,
  confirm,
  confirmColor,
  required,
  onConfirmed,
  ...props
}: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const [reason, setReason] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (props.opened) {
      setReason('');
    }
  }, [props.opened]);

  const submit = () => {
    setLoading(true);
    onConfirmed(reason.trim())
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal {...props}>
      <Stack>
        <Text size='sm'>{description}</Text>
        <TextArea
          withAsterisk={required}
          autosize
          minRows={3}
          maxLength={500}
          label={t('pages.admin.partners.page.modal.reasonLabel', {})}
          description={t('pages.admin.partners.page.modal.reasonHint', {})}
          value={reason}
          onChange={(event) => setReason(event.currentTarget.value)}
        />
        <ModalFooter>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.cancel', {})}
          </Button>
          <Button
            color={confirmColor}
            loading={loading}
            disabled={required && reason.trim().length === 0}
            onClick={submit}
          >
            {confirm}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
