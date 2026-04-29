const API = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error ?? `HTTP ${res.status}`);
  }
  return res.json() as Promise<T>;
}

export interface Campaign {
  id: number;
  merchant: string;
  name?: string | null;
  reward_amount: number;
  expiration: number;
  active: boolean;
  total_claimed: number;
  image_url?: string;
}

export interface Reward {
  id: string;
  user_address: string;
  campaign_id: number;
  amount: number;
  redeemed: boolean;
  redeemed_amount: number;
  claimed_at: string;
}

export interface AnalyticsData {
  totalClaims: number;
  totalLYT: number;
  redemptionRate: number;
  claimsPerCampaign: { name: string; claims: number }[];
  claimsOverTime: { date: string; claims: number }[];
}

export interface TransactionRecord {
  tx_hash: string;
  type: string;
  user_address: string;
  campaign_id: number | null;
  campaign_name: string | null;
  amount: number;
  ledger: number;
  created_at: string;
}

export const api = {
  getCampaigns: (limit = 20, offset = 0) =>
    apiFetch<{ campaigns: Campaign[]; total: number }>(`/campaigns?limit=${limit}&offset=${offset}`),
  getCampaign: (id: number) => apiFetch<{ campaign: Campaign }>(`/campaigns/${id}`),
  getUserRewards: (address: string) =>
    apiFetch<{ rewards: Reward[] }>(`/user/${address}/rewards`),
  getUserTransactions: (address: string, limit = 20, offset = 0) =>
    apiFetch<{ transactions: TransactionRecord[]; total: number }>(`/user/${address}/transactions?limit=${limit}&offset=${offset}`),
  getAnalytics: (days: number) =>
    apiFetch<AnalyticsData>(`/analytics?days=${days}`),
  uploadCampaignImage: (file: File) => {
    const formData = new FormData();
    formData.append("image", file);
    return apiFetch<{ imageUrl: string }>("/campaigns/upload", {
      method: "POST",
      body: formData,
      headers: {}, // fetch will set the correct boundary if body is FormData
    });
  },
  mapCampaignImage: (txHash: string, imageUrl: string) =>
    apiFetch<{ ok: boolean }>("/campaigns/map-image", {
      method: "POST",
      body: JSON.stringify({ txHash, imageUrl }),
    }),
};
