import { Center, Stack, Text } from '@mantine/core';
import { makeComponentHookable } from 'shared';
import Button from '@/elements/buttons/Button.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

function ErrorState({ error, onRetry, retrying }: { error: string; onRetry?: () => void; retrying?: boolean }) {
  const { t } = useTranslations();

  return (
    <Center py='lg'>
      {/* plugged.host: Theme v2 drops the glyph; the heading carries the colour */}
      <Stack align='center' gap='xs'>
        <Text fw={600} c='red.4'>
          {t('common.alert.error', {})}
        </Text>
        <Text c='dimmed' size='sm' ta='center'>
          {error}
        </Text>
        {onRetry && (
          <Button variant='default' size='xs' mt='xs' loading={retrying} onClick={onRetry}>
            {t('common.button.retry', {})}
          </Button>
        )}
      </Stack>
    </Center>
  );
}

export default makeComponentHookable(ErrorState);
