// plugged.host: Admin › Settings › Economy › Tokens, the profile badges' labels, colours and
// optional Discord roles (docs/features/promos.md#profile-badges).
import { UseFormReturnType } from '@mantine/form';
import { z } from 'zod';
import Badge from '@/elements/data-display/Badge.tsx';
import Select from '@/elements/input/Select.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { PROFILE_BADGE_COLORS, PROFILE_BADGE_LABELS, profileBadgeLabelKey } from '@/elements/ProfileBadge.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminSettingsEconomySchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type EconomyValues = z.infer<typeof adminSettingsEconomySchema>;
type BadgeValue = EconomyValues['badges'][number];

const page = 'pages.admin.settings.tabs.economy.page.form';
const COLORS = [
  'blue',
  'yellow',
  'violet',
  'orange',
  'teal',
  'green',
  'red',
  'pink',
  'grape',
  'indigo',
  'cyan',
  'lime',
  'gray',
];

export default function BadgesField({ form }: { form: UseFormReturnType<EconomyValues> }) {
  const { t } = useTranslations();
  const badges = form.getValues().badges;

  const update = (index: number, patch: Partial<BadgeValue>) => {
    form.setFieldValue(
      'badges',
      badges.map((badge, position) => (position === index ? { ...badge, ...patch } : badge)),
    );
  };

  return (
    <Stack gap='md'>
      <Text size='sm' c='dimmed'>
        {t(`${page}.badgesHint`, {})}
      </Text>

      {badges.map((badge, index) => {
        const labelKey = profileBadgeLabelKey(badge.id);
        const label = badge.label?.trim() || (labelKey ? t(PROFILE_BADGE_LABELS[labelKey], {}) : badge.id);

        return (
          <div key={badge.id} className='theme-eco-card'>
            <div className='flex flex-wrap items-center gap-3'>
              <Badge color={badge.color || PROFILE_BADGE_COLORS[badge.id]} variant='light'>
                {label}
              </Badge>
              <Text size='sm' c='dimmed'>
                {badge.id}
              </Text>
            </div>

            <div className='grid gap-3 sm:grid-cols-3'>
              <TextInput
                label={t(`${page}.badgeLabel`, {})}
                description={t(`${page}.badgeLabelDescription`, {})}
                value={badge.label ?? ''}
                onChange={(event) => update(index, { label: event.currentTarget.value || null })}
                maxLength={32}
              />
              <Select
                label={t(`${page}.badgeColor`, {})}
                data={COLORS.map((color) => ({ value: color, label: color }))}
                value={badge.color}
                onChange={(value) => value && update(index, { color: value })}
                allowDeselect={false}
              />
              <TextInput
                label={t(`${page}.badgeDiscordRole`, {})}
                description={t(`${page}.badgeDiscordRoleDescription`, {})}
                value={badge.discordRole ?? ''}
                onChange={(event) => update(index, { discordRole: event.currentTarget.value || null })}
                placeholder='123456789012345678'
                maxLength={32}
              />
            </div>
          </div>
        );
      })}
    </Stack>
  );
}
