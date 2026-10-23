// plugged.host: Admin › Settings › Games, the games players can pick and their software choices
import { UseFormReturnType } from '@mantine/form';
import { z } from 'zod';
import Button from '@/elements/buttons/Button.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Divider from '@/elements/layout/Divider.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminSettingsGamesSchema } from '@/lib/schemas/admin/settings.ts';
import { useGroupedEggOptions } from '@/plugins/useGroupedEggOptions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type GamesValues = z.infer<typeof adminSettingsGamesSchema>;

const emptyVariant = { value: '', label: '', startup: null, image: null };

export default function GameCatalogField({ form }: { form: UseFormReturnType<GamesValues> }) {
  const { t } = useTranslations();
  const { eggOptions, loading } = useGroupedEggOptions();
  const catalog = form.getValues().catalog;

  const used = new Set(catalog.map((game) => game.eggUuid));

  return (
    <Stack gap='md'>
      <Text size='sm' c='dimmed'>
        {t('pages.admin.settings.tabs.games.page.catalog.description', {})}
      </Text>

      {catalog.map((game, index) => (
        <Stack key={form.key(`catalog.${index}`)} gap='xs'>
          {index > 0 && <Divider />}

          <Group align='flex-end' wrap='nowrap'>
            <Select
              className='flex-1'
              label={t('pages.admin.settings.tabs.games.page.catalog.egg', {})}
              placeholder={t('pages.admin.settings.tabs.games.page.catalog.eggPlaceholder', {})}
              data={eggOptions.map((group) => ({
                group: group.group,
                items: group.items.filter((item) => item.value === game.eggUuid || !used.has(item.value)),
              }))}
              searchable
              disabled={loading}
              key={form.key(`catalog.${index}.eggUuid`)}
              {...form.getInputProps(`catalog.${index}.eggUuid`)}
            />
            <Button variant='default' onClick={() => form.removeListItem('catalog', index)}>
              {t('pages.admin.settings.tabs.games.page.catalog.removeGame', {})}
            </Button>
          </Group>

          <Group grow align='flex-start'>
            <TextInput
              label={t('pages.admin.settings.tabs.games.page.catalog.name', {})}
              placeholder={t('pages.admin.settings.tabs.games.page.catalog.namePlaceholder', {})}
              key={form.key(`catalog.${index}.name`)}
              {...form.getInputProps(`catalog.${index}.name`)}
            />
            <TextInput
              label={t('pages.admin.settings.tabs.games.page.catalog.variable', {})}
              description={t('pages.admin.settings.tabs.games.page.catalog.variableDescription', {})}
              placeholder='MC_SOFTWARE'
              key={form.key(`catalog.${index}.variable`)}
              {...form.getInputProps(`catalog.${index}.variable`)}
            />
          </Group>

          <TextInput
            label={t('common.form.description', {})}
            key={form.key(`catalog.${index}.description`)}
            {...form.getInputProps(`catalog.${index}.description`)}
          />

          <Text size='sm' fw={500}>
            {t('pages.admin.settings.tabs.games.page.catalog.variants', {})}
          </Text>
          {game.variants.length === 0 && (
            <Text size='sm' c='dimmed'>
              {t('pages.admin.settings.tabs.games.page.catalog.noVariants', {})}
            </Text>
          )}
          {game.variants.map((_, variantIndex) => (
            <Group key={form.key(`catalog.${index}.variants.${variantIndex}`)} align='flex-end' wrap='nowrap'>
              <TextInput
                className='flex-1'
                label={t('pages.admin.settings.tabs.games.page.catalog.variantValue', {})}
                placeholder='purpur'
                key={form.key(`catalog.${index}.variants.${variantIndex}.value`)}
                {...form.getInputProps(`catalog.${index}.variants.${variantIndex}.value`)}
              />
              <TextInput
                className='flex-1'
                label={t('pages.admin.settings.tabs.games.page.catalog.variantLabel', {})}
                placeholder='Purpur'
                key={form.key(`catalog.${index}.variants.${variantIndex}.label`)}
                {...form.getInputProps(`catalog.${index}.variants.${variantIndex}.label`)}
              />
              <TextInput
                className='flex-1'
                label={t('pages.admin.settings.tabs.games.page.catalog.variantStartup', {})}
                placeholder={t('pages.admin.settings.tabs.games.page.catalog.eggDefault', {})}
                key={form.key(`catalog.${index}.variants.${variantIndex}.startup`)}
                {...form.getInputProps(`catalog.${index}.variants.${variantIndex}.startup`)}
              />
              <TextInput
                className='flex-1'
                label={t('pages.admin.settings.tabs.games.page.catalog.variantImage', {})}
                placeholder={t('pages.admin.settings.tabs.games.page.catalog.eggDefault', {})}
                key={form.key(`catalog.${index}.variants.${variantIndex}.image`)}
                {...form.getInputProps(`catalog.${index}.variants.${variantIndex}.image`)}
              />
              <Button variant='default' onClick={() => form.removeListItem(`catalog.${index}.variants`, variantIndex)}>
                {t('common.button.remove', {})}
              </Button>
            </Group>
          ))}
          <Group>
            <Button
              variant='default'
              size='xs'
              onClick={() => form.insertListItem(`catalog.${index}.variants`, { ...emptyVariant })}
            >
              {t('pages.admin.settings.tabs.games.page.catalog.addVariant', {})}
            </Button>
          </Group>
        </Stack>
      ))}

      <Group>
        <Button
          variant='default'
          onClick={() =>
            form.insertListItem('catalog', { eggUuid: '', name: null, description: null, variable: null, variants: [] })
          }
        >
          {t('pages.admin.settings.tabs.games.page.catalog.addGame', {})}
        </Button>
      </Group>
    </Stack>
  );
}
