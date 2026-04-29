"use client";

import { useEffect, useState } from "react";
import { useWallet } from "@/context/WalletContext";
import { api, Campaign } from "@/lib/api";
import { CampaignTable } from "@/components/CampaignTable";
import { CreateCampaignForm } from "@/components/CreateCampaignForm";
import { deactivateCampaign } from "@/lib/soroban";

export default function MerchantPage() {
  const { publicKey } = useWallet();
  const [campaigns, setCampaigns] = useState<Campaign[]>([]);

  const loadCampaigns = async () => {
    const r = await api.getCampaigns(100, 0);
    if (publicKey) {
      setCampaigns(r.campaigns.filter((c) => c.merchant === publicKey));
    } else {
      setCampaigns(r.campaigns);
    }
  };

  useEffect(() => { loadCampaigns().catch(console.error); }, [publicKey]);

  const handleDeactivate = async (id: number) => {
    if (!publicKey) throw new Error("Wallet not connected");
    // Submit deactivation transaction via Freighter
    await deactivateCampaign(id, publicKey);
    // Optimistically update UI
    setCampaigns((prev) => prev.map((c) => (c.id === id ? { ...c, active: false } : c)));
  };

  return (
    <div>
      <h1 className="page-title">Merchant Portal</h1>

      {!publicKey && (
        <div className="alert alert-error">Connect your Freighter wallet to create campaigns.</div>
      )}

      <section style={{ marginBottom: 48 }}>
        <h2 className="section-title">Create Campaign</h2>
        {publicKey ? (
          <CreateCampaignForm publicKey={publicKey} onSuccess={loadCampaigns} />
        ) : null}
      </section>

      <section>
        <h2 className="section-title">My Campaigns</h2>
        <CampaignTable campaigns={campaigns} onDeactivate={handleDeactivate} merchantPublicKey={publicKey ?? undefined} />
      </section>
    </div>
  );
}
