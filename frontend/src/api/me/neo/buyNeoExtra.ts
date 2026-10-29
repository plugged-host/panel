// plugged.host: one more day's worth of neo usage for tokens, once a window refuses turns
import { axiosInstance } from '@/api/axios.ts';

/** `tokens` is the price the chat showed; a price that moved since answers 409. */
export default async (tokens: number): Promise<void> => {
  await axiosInstance.post('/api/client/account/neo/extra', { tokens });
};
