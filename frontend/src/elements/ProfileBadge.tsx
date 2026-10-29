// plugged.host: a profile badge next to a name (docs/features/promos.md#profile-badges): a flat Mantine
// badge in one of the theme's colours, its label in the reader's language
import Badge from '@/elements/data-display/Badge.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

/** The badges' colours, from Mantine's palette; the ids are shared/src/badges.rs's. */
export const PROFILE_BADGE_COLORS: Record<string, string> = {
  supporter: 'blue',
  early_bird: 'yellow',
  night_owl: 'violet',
  og: 'orange',
  builder: 'teal',
  explorer: 'green',
};

/** "early_bird" → "earlyBird", the label's key. */
export function profileBadgeKey(badge: string): string {
  return badge.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());
}

/** The badges' labels, by id. */
const PROFILE_BADGE_LABELS = {
  supporter: 'elements.profileBadge.supporter',
  early_bird: 'elements.profileBadge.earlyBird',
  night_owl: 'elements.profileBadge.nightOwl',
  og: 'elements.profileBadge.og',
  builder: 'elements.profileBadge.builder',
  explorer: 'elements.profileBadge.explorer',
} as const;

export default function ProfileBadge({ badge, className }: { badge: string | null | undefined; className?: string }) {
  const { t } = useTranslations();
  if (!badge || !PROFILE_BADGE_COLORS[badge] || !Object.hasOwn(PROFILE_BADGE_LABELS, badge)) return null;

  return (
    <Badge size='xs' variant='light' color={PROFILE_BADGE_COLORS[badge]} className={className}>
      {t(PROFILE_BADGE_LABELS[badge as keyof typeof PROFILE_BADGE_LABELS], {})}
    </Badge>
  );
}
