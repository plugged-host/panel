// plugged.host: "Add player": whitelist a player, make them an operator, or both
// (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { usePlayerChange } from './usePlayerChange.ts';

const NAME = /^[A-Za-z0-9_]{1,16}$/;

type Access = 'whitelist' | 'operator' | 'both';

type Props = ModalProps & {
  onlineMode: boolean;
};

export default function AddPlayerModal({ onlineMode, ...props }: Props) {
  const { t } = useTranslations();
  const change = usePlayerChange();
  const base = 'pages.server.minecraft.players';

  const [name, setName] = useState('');
  const [access, setAccess] = useState<Access>('whitelist');
  const [loading, setLoading] = useState(false);

  const close = () => {
    setName('');
    setAccess('whitelist');
    props.onClose();
  };

  const submit = async () => {
    const player = name.trim();
    setLoading(true);

    const whitelisted = access === 'operator' || (await change('whitelist', true, player));
    const opped = access === 'whitelist' || !whitelisted || (await change('operators', true, player));

    setLoading(false);
    if (whitelisted && opped) close();
  };

  const valid = NAME.test(name.trim());

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
          error={name.trim() && !valid ? t(`${base}.form.nameInvalid`, {}) : undefined}
        />
        <Select
          label={t(`${base}.form.access`, {})}
          data={[
            { value: 'whitelist', label: t(`${base}.form.accessWhitelist`, {}) },
            { value: 'operator', label: t(`${base}.form.accessOperator`, {}) },
            { value: 'both', label: t(`${base}.form.accessBoth`, {}) },
          ]}
          value={access}
          onChange={(value) => value && setAccess(value as Access)}
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
