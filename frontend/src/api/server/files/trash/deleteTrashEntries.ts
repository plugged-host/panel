import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverTrashSelectorSchema } from '@/lib/schemas/server/files.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

const deleteTrashEntriesSchema = z.object({
  selector: serverTrashSelectorSchema,
});

export default async (
  uuid: string,
  selector: z.infer<typeof serverTrashSelectorSchema>,
): Promise<{ deleted: number }> => {
  const { data } = await axiosInstance.delete(`/api/client/servers/${uuid}/files/trash`, {
    data: serializeForApi(deleteTrashEntriesSchema, { selector }),
  });
  return data;
};
