// plugged.host: buy the waiting room's day pass with tokens, at the price shown (409 when it moved)
import { axiosInstance } from '@/api/axios.ts';

export default async (price: number): Promise<void> => {
  await axiosInstance.post('/api/client/account/day-pass', { price });
};
