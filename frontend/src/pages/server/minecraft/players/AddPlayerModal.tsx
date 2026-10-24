// plugged.host: whitelist a player, make them an operator, or both (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { usePlayerChange } from './usePlayerChange.ts';

const NAME = /^[A-Za-z0-9_]{1,16}$/;

type Props = ModalProps & {
  onlineMode: boolean;
};

export default function AddPlayerModal({ onlineMode, ...props }: Props) {
  const { t } = useTranslations();
  const change = usePlayerChange();
  const base = 'pages.server.minecraft.players';

  const [name, setName] = useState('');
  const [whitelist, setWhitelist] = useState(true);
  const [operator, setOperator] = useState(false);
  const [loading, setLoading] = useState(false);

  const close = () => {
    setName('');
    setWhitelist(true);
    setOperator(false);
    props.onClose();
  };

  const submit = async () => {
    setLoading(true);

    const whitelisted = !whitelist || (await change('whitelist', true, name.trim()));
    const opped = !operator || !whitelisted || (await change('operators', true, name.trim()));

    setLoading(false);
    if (whitelisted && opped) close();
  };

  const valid = NAME.test(name.trim()) && (whitelist || operator);

  return (
    <Modal title={t(`${base}.modal.add.title`, {})} {...props} onClose={close}>
      <Stack>
        <TextInput
          withAsterisk
          label={t(`${base}.form.name`, {})}
          description={onlineMode ? t(`${base}.form.nameOnline`, {}) : t(`${base}.form.nameOffline`, {})}
          maxLength={16}
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          error={name.trim() && !NAME.test(name.trim()) ? t(`${base}.form.nameInvalid`, {}) : undefined}
        />
        <Checkbox
          label={t(`${base}.form.whitelist`, {})}
          checked={whitelist}
          onChange={(e) => setWhitelist(e.currentTarget.checked)}
        />
        <Checkbox
          label={t(`${base}.form.operator`, {})}
          description={t(`${base}.modal.op.content`, {})}
          checked={operator}
          onChange={(e) => setOperator(e.currentTarget.checked)}
        />

        <ModalFooter>
          <Button onClick={submit} loading={loading} disabled={!valid}>
            {t(`${base}.button.add`, {})}
          </Button>
          <Button variant='default' onClick={close}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
