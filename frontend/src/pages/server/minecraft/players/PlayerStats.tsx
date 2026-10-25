// plugged.host: a player's statistics from the game's statistics file: play time, kills, deaths,
// distance by way of moving and the top blocks, items and mobs (docs/features/minecraft.md#player-details)
import { z } from 'zod';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import { formatNumber, formatUptimeShort } from '@/lib/format/serverUsage.ts';
import { minecraftPlayerStatsSchema, minecraftStatRankingSchema } from '@/lib/schemas/server/minecraft.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { playerBase } from './playerPage.ts';

type Stats = z.infer<typeof minecraftPlayerStatsSchema>;
type Ranking = z.infer<typeof minecraftStatRankingSchema>;

function Ranked({
  title,
  ranking,
  name,
  amount = (value) => formatNumber(value, 0),
}: {
  title: string;
  ranking: Ranking;
  name?: (id: string, label: string) => string;
  amount?: (value: number) => string;
}) {
  const { t } = useTranslations();

  return (
    <section>
      <h3 className='theme-player-caption'>{title}</h3>
      {ranking.top.length > 0 ? (
        <ol className='theme-player-ranking'>
          {ranking.top.map((count) => (
            <li key={count.id}>
              <span>{name ? name(count.id, count.label) : count.label}</span>
              <span>{amount(count.value)}</span>
            </li>
          ))}
        </ol>
      ) : (
        <span className='theme-placeholder'>{t(`${playerBase}.stats.nothing`, {})}</span>
      )}
    </section>
  );
}

export default function PlayerStats({ stats }: { stats: Stats | null }) {
  const { t } = useTranslations();

  // `minecraft:walk_on_water_one_cm`: the game's ways of moving by name, a mod's by its label
  const way = (id: string, label: string) => {
    switch (id.replace(/^minecraft:/, '').replace(/_one_cm$/, '')) {
      case 'walk':
        return t(`${playerBase}.stats.ways.walk`, {});
      case 'sprint':
        return t(`${playerBase}.stats.ways.sprint`, {});
      case 'crouch':
        return t(`${playerBase}.stats.ways.crouch`, {});
      case 'swim':
        return t(`${playerBase}.stats.ways.swim`, {});
      case 'fall':
        return t(`${playerBase}.stats.ways.fall`, {});
      case 'climb':
        return t(`${playerBase}.stats.ways.climb`, {});
      case 'fly':
        return t(`${playerBase}.stats.ways.fly`, {});
      case 'aviate':
        return t(`${playerBase}.stats.ways.aviate`, {});
      case 'boat':
        return t(`${playerBase}.stats.ways.boat`, {});
      case 'minecart':
        return t(`${playerBase}.stats.ways.minecart`, {});
      case 'horse':
        return t(`${playerBase}.stats.ways.horse`, {});
      case 'pig':
        return t(`${playerBase}.stats.ways.pig`, {});
      case 'strider':
        return t(`${playerBase}.stats.ways.strider`, {});
      case 'walk_on_water':
        return t(`${playerBase}.stats.ways.walkOnWater`, {});
      case 'walk_under_water':
        return t(`${playerBase}.stats.ways.walkUnderWater`, {});
      default:
        return label;
    }
  };

  if (!stats) {
    return (
      <TitleCard title={t(`${playerBase}.stats.title`, {})}>
        <p className='theme-player-note'>{t(`${playerBase}.stats.empty`, {})}</p>
      </TitleCard>
    );
  }

  const tiles = [
    { key: 'playTime', label: t(`${playerBase}.stats.playTime`, {}), value: formatUptimeShort(stats.playTime * 1000) },
    { key: 'playerKills', label: t(`${playerBase}.stats.playerKills`, {}), value: formatNumber(stats.playerKills, 0) },
    { key: 'deaths', label: t(`${playerBase}.stats.deaths`, {}), value: formatNumber(stats.deaths, 0) },
    { key: 'kdr', label: t(`${playerBase}.stats.kdr`, {}), value: formatNumber(stats.kdr, 2) },
    { key: 'mobKills', label: t(`${playerBase}.stats.mobKills`, {}), value: formatNumber(stats.mobKills, 0) },
    {
      key: 'distance',
      label: t(`${playerBase}.stats.distance`, {}),
      value: t(`${playerBase}.stats.blocks`, { count: formatNumber(stats.distance.total, 0) }),
    },
  ];

  return (
    <TitleCard title={t(`${playerBase}.stats.title`, {})}>
      <div className='theme-player-stats'>
        {tiles.map((tile) => (
          <div key={tile.key} className='theme-world-stat'>
            <span className='theme-world-stat-label'>{tile.label}</span>
            <span className='theme-world-stat-value'>{tile.value}</span>
          </div>
        ))}
      </div>

      <div className='theme-player-rankings'>
        <Ranked
          title={t(`${playerBase}.stats.distance`, {})}
          ranking={stats.distance}
          name={way}
          amount={(value) => t(`${playerBase}.stats.blocks`, { count: formatNumber(value, 0) })}
        />
        <Ranked title={t(`${playerBase}.stats.mined`, {})} ranking={stats.mined} />
        <Ranked title={t(`${playerBase}.stats.used`, {})} ranking={stats.used} />
        <Ranked title={t(`${playerBase}.stats.killed`, {})} ranking={stats.killed} />
      </div>
    </TitleCard>
  );
}
