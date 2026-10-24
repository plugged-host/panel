// plugged.host: write the server.cfg form's changes; refused when the file changed since it loaded (docs/features/fivem.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import {
  fivemAceSchema,
  fivemConfigSchema,
  fivemPrincipalSchema,
  fivemResourceSchema,
  fivemSettingsSchema,
} from '@/lib/schemas/server/fivem.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface FivemConfigSave {
  hash: string;
  settings: z.infer<typeof fivemSettingsSchema>;
  resources: z.infer<typeof fivemResourceSchema>[];
  principals: z.infer<typeof fivemPrincipalSchema>[];
  aces: z.infer<typeof fivemAceSchema>[];
}

export default async (uuid: string, save: FivemConfigSave): Promise<z.infer<typeof fivemConfigSchema>> => {
  const { settings } = save;
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/fivem/config`, {
    hash: save.hash,
    settings: {
      hostname: settings.hostname,
      project_name: settings.projectName,
      project_description: settings.projectDescription,
      max_clients: settings.maxClients,
      locale: settings.locale,
      icon: settings.icon,
      game_build: settings.gameBuild,
      license_key: settings.licenseKey,
      steam_key: settings.steamKey,
      tebex_secret: settings.tebexSecret,
      tags: settings.tags,
      database: {
        host: settings.database.host,
        port: settings.database.port,
        user: settings.database.user,
        name: settings.database.name,
        password: settings.database.password,
        key_value: settings.database.keyValue,
      },
    },
    resources: save.resources,
    principals: save.principals,
    aces: save.aces,
  });
  return parseFromApi(fivemConfigSchema, data);
};
