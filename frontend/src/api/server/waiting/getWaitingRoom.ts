// plugged.host: the server's start waiting room
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverWaitingRoomSchema } from '@/lib/schemas/server/waitingRoom.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverWaitingRoomSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/waiting-room`);
  return parseFromApi(serverWaitingRoomSchema, data.room);
};
