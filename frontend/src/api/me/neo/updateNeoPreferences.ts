// plugged.host: the person's neo switches: on, act without asking, the model, the first-use notice
import { axiosInstance } from '@/api/axios.ts';
import {
  NeoPreferences,
  NeoPreferencesUpdate,
  neoPreferencesSchema,
  neoPreferencesUpdateSchema,
} from '@/lib/schemas/user/neo.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (changes: NeoPreferencesUpdate): Promise<NeoPreferences> => {
  const { data } = await axiosInstance.patch(
    '/api/client/account/neo/preferences',
    serializeForApi(neoPreferencesUpdateSchema, changes),
  );
  return parseFromApi(neoPreferencesSchema, data.preferences);
};
