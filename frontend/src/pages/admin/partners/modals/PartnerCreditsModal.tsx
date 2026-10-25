// plugged.host: Admin › Partners: the credits a month, when approving or changing a partnership.
import { ModalProps } from '@mantine/core';
import { useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Props = Omit<ModalProps, 'children'> & {
  description: string;
  confirm: string;
  /** neo's suggestion when approving, the current amount when changing it. */
  initial: number;
  onConfirmed: (credits: number) => Promise<void>;
};

export default function PartnerCreditsModal({ description, confirm, initial, onConfirmed, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const [credits, setCredits] = useState<number>(initial);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (props.opened) {
      setCredits(initial);
    }
  }, [props.opened, initial]);

  const submit = () => {
    setLoading(true);
    onConfirmed(credits)
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal {...props}>
      <Stack>
        <Text size='sm'>{description}</Text>
        <NumberInput
          label={t('pages.admin.partners.page.modal.creditsLabel', {})}
          min={0}
          allowDecimal={false}
          thousandSeparator
          value={credits}
          onChange={(value) => setCredits(typeof value === 'number' ? value : Number(value) || 0)}
        />
        <ModalFooter>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.cancel', {})}
          </Button>
          <Button loading={loading} onClick={submit}>
            {confirm}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
