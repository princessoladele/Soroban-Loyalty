"use client";

import Link from "next/link";
import { useRef } from "react";
import { Campaign } from "@/lib/api";
import { useI18n } from "@/context/I18nContext";
import { useCountdown } from "@/hooks/useCountdown";
import { Tooltip } from "@/components/Tooltip";

interface Props {
  campaign: Campaign;
  onClaim?: (id: number) => void;
  claiming?: boolean;
  /** If provided, shows a Deactivate button for the campaign owner */
  onDeactivate?: (id: number) => Promise<void>;
  isMerchantOwned?: boolean;
}

export function CampaignCard({ campaign, onClaim, claiming, onDeactivate, isMerchantOwned }: Props) {
  const { t } = useI18n();
  const countdown = useCountdown(campaign.expiration);
  const prevMinuteRef = useRef<number | null>(null);
  const [showConfirm, setShowConfirm] = useState(false);
  const [deactivating, setDeactivating] = useState(false);
  const [deactivateError, setDeactivateError] = useState<string | null>(null);

  const isInactive = !campaign.active;
  const statusKey = isInactive ? "inactive" : countdown.expired ? "expired" : "active";
  const status = t(`campaigns.status.${statusKey}`);
  const canClaim = campaign.active && !countdown.expired;

  const announceMinute =
    !countdown.expired &&
    prevMinuteRef.current !== null &&
    prevMinuteRef.current !== countdown.minutes;
  if (prevMinuteRef.current !== countdown.minutes) {
    prevMinuteRef.current = countdown.minutes;
  }

  const countdownLabel = countdown.expired
    ? "Expired"
    : countdown.days > 0
    ? `${countdown.days}d ${countdown.hours}h ${countdown.minutes}m ${countdown.seconds}s left`
    : `${countdown.hours}h ${countdown.minutes}m ${countdown.seconds}s left`;

  const handleDeactivateConfirm = async () => {
    if (!onDeactivate) return;
    setDeactivating(true);
    setDeactivateError(null);
    try {
      await onDeactivate(campaign.id);
      setShowConfirm(false);
    } catch (err) {
      setDeactivateError(err instanceof Error ? err.message : "Deactivation failed");
    } finally {
      setDeactivating(false);
    }
  };

  return (
    <div className="card" style={{ position: "relative" }}>
      <div className="card-header">
        <span className="badge" data-status={statusKey}>
          {status}
        </span>
        <span className="campaign-id">
          {t("campaigns.details.campaign")} #{campaign.id}
        </span>
      </div>
      <div className="card-body">
        <p>
          <strong>{t("campaigns.details.merchant")}:</strong>{" "}
          <span className="mono">
            {campaign.merchant.slice(0, 8)}…{campaign.merchant.slice(-4)}
          </span>
        </p>
        <p>
          <strong>{t("campaigns.details.reward")}:</strong>{" "}
          <Tooltip content="LYT tokens you earn by claiming this campaign">
            <span>{campaign.reward_amount.toLocaleString()} LYT</span>
          </Tooltip>
        </p>
        <p>
          <strong>{t("campaigns.details.claimed")}:</strong>{" "}
          {campaign.total_claimed}
        </p>
        <p>
          <strong>{t("campaigns.details.expires")}:</strong>{" "}
          <Tooltip content="Time remaining before this campaign expires and can no longer be claimed">
            <span
              aria-live="off"
              aria-label={countdownLabel}
              data-testid="countdown"
            >
              {countdownLabel}
            </span>
          </Tooltip>
          {announceMinute && (
            <span className="sr-only" aria-live="polite" aria-atomic="true">
              {countdownLabel}
            </span>
          )}
        </p>
      </div>

      <div className="card-footer" style={{ display: "flex", gap: 8 }}>
        {onClaim && (
          <Tooltip content="Claim this campaign to earn LYT tokens to your wallet">
            <button
              onClick={() => onClaim(campaign.id)}
              disabled={!canClaim || claiming}
              className="btn btn-primary"
            >
              {claiming
                ? t("campaigns.actions.claiming")
                : t("campaigns.actions.claim")}
            </button>
          </Tooltip>
        )}

        {isMerchantOwned && campaign.active && onDeactivate && (
          <button
            className="btn btn-danger"
            onClick={() => setShowConfirm(true)}
            disabled={deactivating}
            aria-label={`Deactivate campaign #${campaign.id}`}
          >
            {t("campaigns.actions.deactivate")}
          </button>
        )}
      </div>

      {/* Confirmation dialog */}
      {showConfirm && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="deactivate-dialog-title"
          style={{
            position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)",
            display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000,
          }}
        >
          <div className="card" style={{ maxWidth: 400, width: "90%", padding: 24 }}>
            <h2 id="deactivate-dialog-title" style={{ marginBottom: 12 }}>
              {t("campaigns.deactivate.title")}
            </h2>
            <p style={{ marginBottom: 16 }}>{t("campaigns.deactivate.warning")}</p>
            {deactivateError && (
              <p className="alert alert-error" style={{ marginBottom: 12 }}>{deactivateError}</p>
            )}
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button
                className="btn btn-secondary"
                onClick={() => { setShowConfirm(false); setDeactivateError(null); }}
                disabled={deactivating}
              >
                {t("common.cancel")}
              </button>
              <button
                className="btn btn-danger"
                onClick={handleDeactivateConfirm}
                disabled={deactivating}
              >
                {deactivating ? t("campaigns.actions.deactivating") : t("common.confirm")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface Props {
  campaign: Campaign;
  onClaim?: (id: number) => void;
  claiming?: boolean;
}

export function CampaignCard({ campaign, onClaim, claiming }: Props) {
  const { t } = useI18n();
  const countdown = useCountdown(campaign.expiration);
  const prevMinuteRef = useRef<number | null>(null);

  const isInactive = !campaign.active;
  const statusKey = isInactive ? "inactive" : countdown.expired ? "expired" : "active";
  const status = t(`campaigns.status.${statusKey}`);
  const canClaim = campaign.active && !countdown.expired;

  // Announce to screen readers on each minute change
  const announceMinute =
    !countdown.expired &&
    prevMinuteRef.current !== null &&
    prevMinuteRef.current !== countdown.minutes;
  if (prevMinuteRef.current !== countdown.minutes) {
    prevMinuteRef.current = countdown.minutes;
  }

  const countdownLabel = countdown.expired
    ? "Expired"
    : countdown.days > 0
    ? `${countdown.days}d ${countdown.hours}h ${countdown.minutes}m ${countdown.seconds}s left`
    : `${countdown.hours}h ${countdown.minutes}m ${countdown.seconds}s left`;

  return (
    <Link
      href={`/campaigns/${campaign.id}`}
      style={{ textDecoration: 'none', color: 'inherit', display: 'block' }}
      aria-label={`View campaign ${campaign.id} details`}
    >
      <div className="card" style={{ position: "relative", cursor: 'pointer', transition: 'transform 0.2s, box-shadow 0.2s' }}
        onMouseEnter={(e) => {
          e.currentTarget.style.transform = 'translateY(-2px)';
          e.currentTarget.style.boxShadow = '0 4px 12px rgba(0,0,0,0.15)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.transform = 'none';
          e.currentTarget.style.boxShadow = '';
        }}
      >
        <div className="card-header">
          <span className="badge" data-status={statusKey}>
            {status}
          </span>
          <span className="campaign-id">
            {t("campaigns.details.campaign")} #{campaign.id}
          </span>
        </div>
        <div className="card-body">
          <p>
            <strong>{t("campaigns.details.merchant")}:</strong>{" "}
            <span className="mono">
              {campaign.merchant.slice(0, 8)}…{campaign.merchant.slice(-4)}
            </span>
          </p>
          <p>
            <strong>{t("campaigns.details.reward")}:</strong>{" "}
            <Tooltip content="LYT tokens you earn by claiming this campaign">
              <span>{campaign.reward_amount.toLocaleString()} LYT</span>
            </Tooltip>
          </p>
          <p>
            <strong>{t("campaigns.details.claimed")}:</strong>{" "}
            {campaign.total_claimed}
          </p>
          <p>
            <strong>{t("campaigns.details.expires")}:</strong>{" "}
            <Tooltip content="Time remaining before this campaign expires and can no longer be claimed">
              <span
                aria-live="off"
                aria-label={countdownLabel}
                data-testid="countdown"
              >
                {countdownLabel}
              </span>
            </Tooltip>
            {/* Announce minute changes to screen readers */}
            {announceMinute && (
              <span className="sr-only" aria-live="polite" aria-atomic="true">
                {countdownLabel}
              </span>
            )}
          </p>
        </div>
        {onClaim && (
          <div className="card-footer" onClick={(e) => e.preventDefault()}>
            <Tooltip content="Claim this campaign to earn LYT tokens to your wallet">
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onClaim(campaign.id);
                }}
                disabled={!canClaim || claiming}
                className="btn btn-primary"
              >
                {claiming
                  ? t("campaigns.actions.claiming")
                  : t("campaigns.actions.claim")}
              </button>
            </Tooltip>
          </div>
        )}
      </div>
    </Link>
  );
}
