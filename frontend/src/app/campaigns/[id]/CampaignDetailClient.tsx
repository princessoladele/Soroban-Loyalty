"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api, Campaign } from "@/lib/api";
import { useWallet } from "@/context/WalletContext";
import { useI18n } from "@/context/I18nContext";
import { useToast } from "@/context/ToastContext";
import { claimReward } from "@/lib/soroban";
import { ShareCampaign } from "@/components/ShareCampaign";
import { CampaignCard } from "@/components/CampaignCard";
import { useCountdown } from "@/hooks/useCountdown";

interface Props {
  id: number;
}

export default function CampaignDetailClient({ id }: Props) {
  const [campaign, setCampaign] = useState<Campaign | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [claiming, setClaiming] = useState(false);
  const { publicKey } = useWallet();
  const { t } = useI18n();
  const { toast } = useToast();
  const router = useRouter();

  const baseUrl = typeof window !== "undefined" ? window.location.origin : "";
  const campaignUrl = `${baseUrl}/campaigns/${id}`;

  useEffect(() => {
    setLoading(true);
    api.getCampaign(id)
      .then((res) => {
        setCampaign(res.campaign);
        setError(null);
      })
      .catch((err) => {
        console.error(err);
        setError(t('common.error'));
      })
      .finally(() => setLoading(false));
  }, [id, t]);

  const handleClaim = async () => {
    if (!publicKey) {
      toast(t('wallet.connectFirst'), "error");
      return;
    }
    setClaiming(true);
    try {
      await claimReward(publicKey, id);
      toast(t('messages.claimSuccess', { id: id.toString() }), "success");
      // Refresh campaign data
      const res = await api.getCampaign(id);
      setCampaign(res.campaign);
    } catch (err: any) {
      toast(err.message || t('messages.claimFailed'), "error");
    } finally {
      setClaiming(false);
    }
  };

  if (loading) return (
    <div className="site-main">
      <div className="container">
        <div style={{ padding: '40px', textAlign: 'center' }}>{t('common.loading')}</div>
      </div>
    </div>
  );

  if (error || !campaign) return (
    <div className="site-main">
      <div className="container">
        <nav className="breadcrumb" style={{ marginBottom: '24px' }}>
          <Link href="/dashboard" className="breadcrumb-link">{t('dashboard.title')}</Link>
          <span className="breadcrumb-separator">/</span>
          <span className="breadcrumb-current">Campaign Not Found</span>
        </nav>
        <div className="alert alert-error" style={{ maxWidth: '600px', margin: '40px auto' }}>
          <h2 style={{ marginTop: 0 }}>Campaign Not Found</h2>
          <p>The campaign you are looking for does not exist or has been removed.</p>
          <Link href="/dashboard" className="btn btn-primary" style={{ marginTop: '16px' }}>
            Back to Campaigns
          </Link>
        </div>
      </div>
    </div>
  );

  const countdown = useCountdown(campaign.expiration);
  const isExpired = countdown.expired;
  const canClaim = campaign.active && !isExpired;

  return (
    <div className="site-main">
      <div className="container">
        <nav className="breadcrumb" style={{ marginBottom: '24px', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Link href="/dashboard" className="breadcrumb-link" style={{ color: 'var(--primary)', textDecoration: 'none' }}>
            {t('dashboard.title')}
          </Link>
          <span className="breadcrumb-separator" style={{ color: 'var(--text-muted)' }}>/</span>
          <span className="breadcrumb-current" style={{ color: 'var(--text)' }}>
            {campaign.name || `Campaign #${id}`}
          </span>
        </nav>

        <div style={{ marginBottom: '32px' }}>
          <h1 className="page-title" style={{ fontSize: '2rem', marginBottom: '8px' }}>
            {campaign.name || `${t('campaigns.details.campaign')} #${id}`}
          </h1>
          {campaign.name && (
            <p style={{ color: 'var(--text-muted)', fontSize: '0.9rem' }}>
              {t('campaigns.details.campaign')} #{id}
            </p>
          )}
        </div>

        <div className="grid" style={{ gridTemplateColumns: '1fr 1fr', gap: '32px' }}>
          <div>
            <div className="card" style={{ marginBottom: '24px' }}>
              <div className="card-header">
                <span className="badge" data-status={!campaign.active ? 'inactive' : isExpired ? 'expired' : 'active'}>
                  {!campaign.active ? t('campaigns.status.inactive') : isExpired ? t('campaigns.status.expired') : t('campaigns.status.active')}
                </span>
              </div>
              <div className="card-body">
                <p>
                  <strong>{t("campaigns.details.merchant")}:</strong>{" "}
                  <span className="mono">{campaign.merchant.slice(0, 8)}…{campaign.merchant.slice(-4)}</span>
                </p>
                <p>
                  <strong>{t("campaigns.details.reward")}:</strong>{" "}
                  <span>{campaign.reward_amount.toLocaleString()} LYT</span>
                </p>
                <p>
                  <strong>{t("campaigns.details.claimed")}:</strong>{" "}
                  {campaign.total_claimed}
                </p>
                <p>
                  <strong>{t("campaigns.details.expires")}:</strong>{" "}
                  <span>
                    {isExpired
                      ? "Expired"
                      : countdown.days > 0
                      ? `${countdown.days}d ${countdown.hours}h ${countdown.minutes}m left`
                      : `${countdown.hours}h ${countdown.minutes}m ${countdown.seconds}s left`}
                  </span>
                </p>
                {campaign.name && (
                  <p style={{ marginTop: '16px', padding: '12px', background: 'var(--bg-surface)', borderRadius: '8px' }}>
                    <strong>Description:</strong> {campaign.name}
                  </p>
                )}
              </div>
              <div className="card-footer">
                <button
                  onClick={handleClaim}
                  disabled={!canClaim || claiming}
                  className="btn btn-primary"
                  style={{ width: '100%' }}
                >
                  {claiming
                    ? t("campaigns.actions.claiming")
                    : t("campaigns.actions.claim")}
                </button>
                {!canClaim && campaign.active && isExpired && (
                  <p style={{ marginTop: '8px', fontSize: '0.85rem', color: 'var(--text-muted)' }}>
                    This campaign has expired
                  </p>
                )}
                {!campaign.active && (
                  <p style={{ marginTop: '8px', fontSize: '0.85rem', color: 'var(--text-muted)' }}>
                    This campaign is inactive
                  </p>
                )}
              </div>
            </div>

            <Link href="/dashboard" className="btn btn-outline" style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
              ← Back to Campaigns
            </Link>
          </div>

          <section>
            <h2 className="section-title" style={{ fontSize: '1.2rem', marginBottom: '16px' }}>{t('sharing.title')}</h2>
            <ShareCampaign campaignId={id} url={campaignUrl} />
          </section>
        </div>
      </div>
    </div>
  );
}
