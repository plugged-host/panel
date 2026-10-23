import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverDirectoryEntrySchema, serverTrashEntrySchema } from '@/lib/schemas/server/files.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

interface Props {
  uuid: string;
  entries: {
    uuid: string;
    path?: string;
  }[];
  overwrite?: boolean;
}

interface Result {
  restored: z.infer<typeof serverTrashEntrySchema>[];
  conflicts: {
    uuid: string;
    destination: z.infer<typeof serverDirectoryEntrySchema>;
  }[];
}

export default async ({ uuid, entries, overwrite = false }: Props): Promise<Result> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/files/trash/restore`, {
    entries,
    overwrite,
  });
  return {
    restored: data.restored.map((entry: unknown) => parseFromApi(serverTrashEntrySchema, entry)),
    conflicts: data.conflicts.map((conflict: { uuid: string; destination: unknown }) => ({
      uuid: conflict.uuid,
      destination: parseFromApi(serverDirectoryEntrySchema, conflict.destination),
    })),
  };
};
