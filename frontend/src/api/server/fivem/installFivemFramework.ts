// plugged.host: start installing a framework; it runs in the background (docs/features/fivem.md#frameworks)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { FivemDatabaseChoice, fivemFrameworkInstallSchema } from '@/lib/schemas/server/fivem.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface FivemFrameworkInstall {
  recipe: string;
  serverName: string;
  license: string;
  database: FivemDatabaseChoice | null;
}

export default async (
  uuid: string,
  install: FivemFrameworkInstall,
): Promise<z.infer<typeof fivemFrameworkInstallSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/fivem/frameworks`, {
    recipe: install.recipe,
    server_name: install.serverName,
    license: install.license,
    database: install.database,
  });
  return parseFromApi(fivemFrameworkInstallSchema, data.install);
};
