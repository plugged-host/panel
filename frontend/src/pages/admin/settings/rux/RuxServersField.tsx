// plugged.host: Settings › Accounts › rux, the more Discord servers rux works on (docs/features/rux.md)
import { UseFormReturnType } from '@mantine/form';
import { z } from 'zod';
import Button from '@/elements/buttons/Button.tsx';
import TagsInput from '@/elements/input/TagsInput.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Divider from '@/elements/layout/Divider.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminSettingsRuxSchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type RuxValues = z.infer<typeof adminSettingsRuxSchema>;

/** Servers rux works on besides the main one (shared/src/settings/rux.rs MAX_SERVERS). */
export const MAX_RUX_SERVERS = 9;

const emptyServer = {
  id: '',
  name: '',
  about: '',
  giveawayChannelId: '',
  modLogChannelId: '',
  trustedDomains: [],
};

interface Props {
  form: UseFormReturnType<RuxValues>;
  appName: string;
  readOnly: boolean;
}

export default function RuxServersField({ form, appName, readOnly }: Props) {
  const { t } = useTranslations();
  const page = 'pages.admin.settings.tabs.rux.page.form';
  const servers = form.getValues().servers;

  return (
    <Stack gap='md'>
      <Text size='sm' c='dimmed'>
        {t(`${page}.serversDescription`, {})}
      </Text>

      {servers.map((_, index) => (
        <Stack key={form.key(`servers.${index}`)} gap='xs'>
          {index > 0 && <Divider />}

          <Group align='flex-end' wrap='nowrap'>
            <TextInput
              className='flex-1'
              label={t(`${page}.server`, { number: index + 1 })}
              description={t(`${page}.serverIdDescription`, {})}
              withAsterisk
              key={form.key(`servers.${index}.id`)}
              {...form.getInputProps(`servers.${index}.id`)}
            />
            {!readOnly && (
              <Button variant='default' onClick={() => form.removeListItem('servers', index)}>
                {t('common.button.remove', {})}
              </Button>
            )}
          </Group>

          <Group grow align='flex-start'>
            <TextInput
              label={t(`${page}.serverName`, {})}
              description={t(`${page}.serverNameDescription`, { app: appName })}
              key={form.key(`servers.${index}.name`)}
              {...form.getInputProps(`servers.${index}.name`)}
            />
            <TextInput
              label={t(`${page}.serverAbout`, {})}
              description={t(`${page}.serverAboutDescription`, {})}
              key={form.key(`servers.${index}.about`)}
              {...form.getInputProps(`servers.${index}.about`)}
            />
          </Group>

          <Group grow align='flex-start'>
            <TextInput
              label={t(`${page}.giveawayChannelId`, {})}
              key={form.key(`servers.${index}.giveawayChannelId`)}
              {...form.getInputProps(`servers.${index}.giveawayChannelId`)}
            />
            <TextInput
              label={t(`${page}.modLogChannelId`, {})}
              key={form.key(`servers.${index}.modLogChannelId`)}
              {...form.getInputProps(`servers.${index}.modLogChannelId`)}
            />
          </Group>

          <TagsInput
            label={t(`${page}.serverTrustedDomains`, {})}
            description={t(`${page}.serverTrustedDomainsDescription`, {})}
            placeholder='example.com'
            key={form.key(`servers.${index}.trustedDomains`)}
            value={form.getValues().servers[index]?.trustedDomains ?? []}
            onChange={(domains) => form.setFieldValue(`servers.${index}.trustedDomains`, domains)}
            error={form.errors[`servers.${index}.trustedDomains`]}
          />
        </Stack>
      ))}

      {!readOnly && servers.length < MAX_RUX_SERVERS && (
        <Group>
          <Button variant='default' onClick={() => form.insertListItem('servers', { ...emptyServer })}>
            {t(`${page}.addServer`, {})}
          </Button>
        </Group>
      )}
    </Stack>
  );
}
