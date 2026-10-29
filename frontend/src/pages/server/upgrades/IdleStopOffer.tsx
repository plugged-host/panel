// plugged.host: the console of a server limbo stopped for being idle offers a keep-awake pass while it is
// off (docs/features/server-upgrades.md, docs/features/limbo.md)
import { useServerStore } from '@/stores/server.ts';
import UpgradeOfferCallout from './UpgradeOfferCallout.tsx';

export default function IdleStopOffer() {
  const server = useServerStore((store) => store.server);
  const power = useServerStore((store) => store.state);

  return (
    <UpgradeOfferCallout
      kind='keep_awake'
      show={server.limboState === 'stopped' && power === 'offline'}
      className='mb-6'
    />
  );
}
