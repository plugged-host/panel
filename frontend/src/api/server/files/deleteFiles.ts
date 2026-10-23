import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverTrashEntrySchema } from '@/lib/schemas/server/files.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  root: string,
  files: string[],
  permanent = false,
): Promise<{ deleted: number; entries: z.infer<typeof serverTrashEntrySchema>[] }> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/files/delete`, { root, files, permanent });
  return {
    deleted: data.deleted,
    entries: data.entries.map((entry: unknown) => parseFromApi(serverTrashEntrySchema, entry)),
  };
};
