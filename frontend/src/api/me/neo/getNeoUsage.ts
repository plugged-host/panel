// plugged.host: where the person's limits stand and the models they can pick (null while neo
// cannot tell, e.g. noprofit.space not answering)
import { axiosInstance } from '@/api/axios.ts';
import { NeoUsage, neoUsageSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<NeoUsage | null> => {
  const { data } = await axiosInstance.get('/api/client/account/neo/usage');
  return data.usage ? parseFromApi(neoUsageSchema, data.usage) : null;
};
