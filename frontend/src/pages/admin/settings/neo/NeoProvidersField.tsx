// plugged.host: Admin › Settings › AI › neo, the model slots (Pelican's "Model N" sections): fields, status and a test call
import { UseFormReturnType } from '@mantine/form';
import isEqual from 'fast-deep-equal';
import { useState } from 'react';
import { z } from 'zod';
import testNeoProvider from '@/api/admin/system/neo/testNeoProvider.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import PasswordInput from '@/elements/input/PasswordInput.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Divider from '@/elements/layout/Divider.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConditionalTooltip from '@/elements/overlays/ConditionalTooltip.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { NEO_HEADER_MASK, NEO_MAX_PROVIDERS, nextFreeSlot } from '@/lib/neo.ts';
import { adminNeoProviderStatusSchema, adminNeoProviderTestSchema } from '@/lib/schemas/admin/neo.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import { type NeoProviderValues, type NeoValues, neoProviderToFormValues, newNeoProvider } from './neoFormValues.ts';

type ProviderStatus = z.infer<typeof adminNeoProviderStatusSchema>;
type TestResult = z.infer<typeof adminNeoProviderTestSchema>;

const page = 'pages.admin.settings.tabs.neo.page';
const models = 'pages.admin.settings.tabs.neo.page.models';

const monospace = { input: { fontFamily: 'var(--mantine-font-family-monospace)' } };

interface Props {
  form: UseFormReturnType<NeoValues>;
  statuses: ProviderStatus[];
}

export default function NeoProvidersField({ form, statuses }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const stored = useAdminStore((state) => state.neo.providers);
  const economy = useAdminStore((state) => state.economy);
  const [testing, setTesting] = useState<number | null>(null);
  const [results, setResults] = useState<Record<number, TestResult>>({});

  const providers = form.getValues().providers;

  // the test calls the slot as saved, so only a slot without unsaved changes can be tested
  const isSaved = (provider: NeoProviderValues) => {
    const saved = stored.find((candidate) => candidate.slot === provider.slot);
    return !!saved && isEqual(neoProviderToFormValues(saved), provider);
  };

  const forget = (slot: number) => setResults(({ [slot]: _forgotten, ...rest }) => rest);

  const test = (slot: number) => {
    setTesting(slot);
    forget(slot);

    testNeoProvider(slot)
      .then((result) => setResults((results) => ({ ...results, [slot]: result })))
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setTesting(null));
  };

  const add = () => {
    const slot = nextFreeSlot(providers.map((provider) => provider.slot));
    if (slot === null) {
      return;
    }

    // the list stays in slot order, the order the models are offered in
    form.insertListItem('providers', newNeoProvider(slot), providers.filter((p) => p.slot < slot).length);
  };

  const remove = (index: number, slot: number) => {
    forget(slot);
    form.removeListItem('providers', index);
  };

  // Pelican's "Tokens today: N. Skipped until … (error)." under the section title
  const statusLine = (provider: NeoProviderValues) => {
    const status = statuses.find((candidate) => candidate.slot === provider.slot);
    if (!status || !isSaved(provider)) {
      return null;
    }

    const parts = [t(`${page}.status.tokensToday`, { tokens: status.tokensToday.toLocaleString() })];
    if (status.coolingDownUntil !== null) {
      parts.push(
        t(`${page}.status.coolingDown`, {
          time: formatDateTime(status.coolingDownUntil * 1000),
          error: status.lastError?.reason ?? '?',
        }),
      );
    } else if (status.lastError) {
      parts.push(
        t(`${page}.status.lastError`, {
          time: formatDateTime(status.lastError.at * 1000),
          error: status.lastError.reason,
        }),
      );
    }
    if (provider.enabled && !status.usable) {
      parts.push(t(`${page}.status.notUsable`, {}));
    }

    return parts.join(' ');
  };

  const path = (index: number, field: keyof NeoProviderValues) => `providers.${index}.${field}`;

  return (
    <Stack gap='md'>
      <Text size='sm' c='dimmed'>
        {t(`${models}.hint`, {})}
      </Text>

      {providers.length === 0 && (
        <Text size='sm' c='dimmed'>
          {t(`${models}.noProviders`, {})}
        </Text>
      )}

      {providers.map((provider, index) => {
        const saved = isSaved(provider);
        const status = statusLine(provider);
        const result = results[provider.slot];

        return (
          <Stack key={provider.slot} gap='md'>
            <Divider
              label={
                provider.label.trim()
                  ? t(`${models}.slotTitleLabel`, { slot: provider.slot, label: provider.label.trim() })
                  : t(`${models}.slotTitle`, { slot: provider.slot })
              }
              labelPosition='left'
            />

            {status && (
              <Text size='sm' c='dimmed'>
                {status}
              </Text>
            )}

            <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
              <Switch
                label={t(`${models}.enabled`, {})}
                key={form.key(path(index, 'enabled'))}
                {...form.getInputProps(path(index, 'enabled'), { type: 'checkbox' })}
              />
              <TextInput
                label={t(`${models}.label`, {})}
                description={t(`${models}.labelDescription`, {})}
                withAsterisk={provider.enabled}
                key={form.key(path(index, 'label'))}
                {...form.getInputProps(path(index, 'label'))}
              />
              <TextInput
                label={t(`${models}.description`, {})}
                description={t(`${models}.descriptionDescription`, {})}
                key={form.key(path(index, 'description'))}
                {...form.getInputProps(path(index, 'description'))}
              />
              <Select
                label={t(`${models}.access`, {})}
                data={[
                  { value: 'everyone', label: t(`${models}.accessEveryone`, {}) },
                  {
                    value: 'plus',
                    label: t(`${models}.accessPlus`, { plus: economy.plusName, plusplus: economy.plusplusName }),
                  },
                  { value: 'plusplus', label: t(`${models}.accessPlusplus`, { plusplus: economy.plusplusName }) },
                ]}
                key={form.key(path(index, 'access'))}
                {...form.getInputProps(path(index, 'access'))}
              />
              <Select
                label={t(`${models}.driver`, {})}
                data={[
                  { value: 'openai', label: t(`${models}.driverOpenai`, {}) },
                  { value: 'anthropic', label: t(`${models}.driverAnthropic`, {}) },
                  { value: 'gemini', label: t(`${models}.driverGemini`, {}) },
                ]}
                key={form.key(path(index, 'driver'))}
                {...form.getInputProps(path(index, 'driver'))}
              />
              <TextInput
                label={t(`${models}.model`, {})}
                description={t(`${models}.modelDescription`, {})}
                withAsterisk={provider.enabled}
                key={form.key(path(index, 'model'))}
                {...form.getInputProps(path(index, 'model'))}
              />
              <TextInput
                className='col-span-full'
                label={t(`${models}.endpoint`, {})}
                description={t(`${models}.endpointDescription`, {})}
                placeholder='https://api.example.com/v1'
                withAsterisk={provider.enabled}
                key={form.key(path(index, 'endpoint'))}
                {...form.getInputProps(path(index, 'endpoint'))}
              />
              <PasswordInput
                label={t(`${models}.apiKey`, {})}
                description={
                  !provider.keyStored
                    ? t(`${models}.apiKeyMissing`, {})
                    : provider.clearApiKey
                      ? t(`${models}.apiKeyRemoved`, {})
                      : t(`${models}.apiKeyStored`, {})
                }
                autoComplete='new-password'
                disabled={provider.clearApiKey}
                key={form.key(path(index, 'apiKey'))}
                {...form.getInputProps(path(index, 'apiKey'))}
              />
              {provider.keyStored && (
                <Switch
                  label={t(`${models}.clearApiKey`, {})}
                  key={form.key(path(index, 'clearApiKey'))}
                  {...form.getInputProps(path(index, 'clearApiKey'), { type: 'checkbox' })}
                />
              )}
              <NumberInput
                label={t(`${models}.maxTokens`, {})}
                min={256}
                max={128000}
                withAsterisk
                key={form.key(path(index, 'maxTokens'))}
                {...form.getInputProps(path(index, 'maxTokens'))}
              />
              <NumberInput
                label={t(`${models}.timeout`, {})}
                description={t(`${models}.timeoutDescription`, {})}
                min={5}
                max={300}
                withAsterisk
                key={form.key(path(index, 'timeout'))}
                {...form.getInputProps(path(index, 'timeout'))}
              />
              <NumberInput
                label={t(`${models}.dailyTokenCap`, {})}
                description={t(`${models}.dailyTokenCapDescription`, {})}
                min={0}
                withAsterisk
                key={form.key(path(index, 'dailyTokenCap'))}
                {...form.getInputProps(path(index, 'dailyTokenCap'))}
              />
              <Select
                label={t(`${models}.vision`, {})}
                description={t(`${models}.visionDescription`, {})}
                data={[
                  { value: 'auto', label: t(`${models}.visionAuto`, {}) },
                  { value: 'yes', label: t(`${models}.visionYes`, {}) },
                  { value: 'no', label: t(`${models}.visionNo`, {}) },
                ]}
                key={form.key(path(index, 'vision'))}
                {...form.getInputProps(path(index, 'vision'))}
              />
              <NumberInput
                label={t(`${models}.multiplier`, {})}
                description={t(`${models}.multiplierDescription`, {})}
                min={1}
                max={10}
                withAsterisk
                key={form.key(path(index, 'multiplier'))}
                {...form.getInputProps(path(index, 'multiplier'))}
              />
              <TextArea
                className='col-span-full'
                label={t(`${models}.extraHeaders`, {})}
                description={t(`${models}.extraHeadersDescription`, {
                  example: '{"HTTP-Referer": "https://panel.example.com"}',
                  mask: NEO_HEADER_MASK,
                })}
                placeholder='{}'
                autosize
                minRows={3}
                spellCheck={false}
                styles={monospace}
                key={form.key(path(index, 'extraHeaders'))}
                {...form.getInputProps(path(index, 'extraHeaders'))}
              />
              <TextArea
                className='col-span-full'
                label={t(`${models}.extraBody`, {})}
                description={t(`${models}.extraBodyDescription`, {
                  example: '{"reasoning_format": "hidden"}',
                  gemini: '{"generationConfig": {"temperature": 0.7}}',
                })}
                placeholder='{}'
                autosize
                minRows={3}
                spellCheck={false}
                styles={monospace}
                key={form.key(path(index, 'extraBody'))}
                {...form.getInputProps(path(index, 'extraBody'))}
              />
            </div>

            <Group>
              <AdminCan action='settings.update'>
                <ConditionalTooltip enabled={!saved} label={t(`${page}.test.saveFirst`, {})}>
                  <Button
                    variant='default'
                    loading={testing === provider.slot}
                    disabled={!saved || (testing !== null && testing !== provider.slot)}
                    onClick={() => test(provider.slot)}
                  >
                    {t(`${page}.test.button`, { slot: provider.slot })}
                  </Button>
                </ConditionalTooltip>
              </AdminCan>
              <Button variant='default' onClick={() => remove(index, provider.slot)}>
                {t('common.button.remove', {})}
              </Button>
            </Group>

            {result &&
              (result.successful ? (
                <Notification className='theme-callout' color='green'>
                  <Stack gap='xs'>
                    {t(`${page}.test.successful`, {
                      slot: provider.slot,
                      ms: result.durationMs,
                      model: result.model,
                      tokens: result.tokens.toLocaleString(),
                    })}
                    {result.answer && (
                      <Code block className='whitespace-pre-wrap break-all'>
                        {result.answer}
                      </Code>
                    )}
                  </Stack>
                </Notification>
              ) : (
                <Notification className='theme-callout' color='red'>
                  <Stack gap='xs'>
                    {t(`${page}.test.failed`, { slot: provider.slot, ms: result.durationMs })}
                    {result.error && (
                      <Code block className='whitespace-pre-wrap break-all'>
                        {result.status ? `${result.status}: ${result.error}` : result.error}
                      </Code>
                    )}
                  </Stack>
                </Notification>
              ))}
          </Stack>
        );
      })}

      <Group>
        <Button variant='default' disabled={providers.length >= NEO_MAX_PROVIDERS} onClick={add}>
          {t(`${models}.add`, {})}
        </Button>
      </Group>
    </Stack>
  );
}
