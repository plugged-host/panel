// plugged.host: a new world in place of the loaded one (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftGenerationSchema, minecraftReplacedSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  generation: z.infer<typeof minecraftGenerationSchema>,
  keep: boolean,
): Promise<z.infer<typeof minecraftReplacedSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/worlds`, {
    name: generation.name,
    seed: generation.seed,
    level_type: generation.levelType,
    structures: generation.structures,
    generator: generation.generator,
    datapacks: generation.datapacks,
    keep,
  });
  return parseFromApi(minecraftReplacedSchema, data);
};
